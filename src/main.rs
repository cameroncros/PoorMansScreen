use clap::{CommandFactory, Parser};
use std::io::{stderr, stdin, stdout, Read, Write};
use std::path::Path;
use std::process::{exit, Stdio};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::process::Command;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, trailing_var_arg=true)]
struct Args {
    /// The tag/label
    #[arg()]
    tag: String,

    /// The optional command
    #[arg()]
    cmd: Option<Vec<String>>,
}


fn socket_path(label: &str) -> String
{
    format!("/tmp/{label}")
}

async fn connect_process(label: &str, stdin: &mut dyn Read)
{
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());

    match UnixStream::connect(socket).await {
        Ok(mut sock) => {io_to_stream(stdin, &mut sock).await}
        Err(e) => {
            panic!("Unable to connect to socket - {e}");
        }
    }
}

async fn io_to_stream(stdin: &mut dyn Read, sock: &mut UnixStream) {
    loop {
        let mut buf = [0u8; 1024];
        match stdin.read(&mut buf) {
            Ok(len) => {
                if len == 0 {
                    break;
                }
                sock.write_all(&buf[..len]).await.expect("Failed to write to unix socket");
            }
            Err(_) => {break}
        }
    }
}

async fn run_process(label: &str, cmd: &[String]) {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());
    if socket.exists() {
        std::fs::remove_file(socket).expect("Failed to remove existing unix socket")
    }
    let mut stream = match UnixListener::bind(socket) {
        Err(_) => panic!("Failed to create unix socket"),
        Ok(stream) => stream,
    };
    let exe = cmd.first().unwrap();
    let args = &cmd[1..];
    let mut command = Command::new(exe);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if !args.is_empty() {
        command.args(args);
    }
    let mut child = command.spawn().expect("Failed to start child process");

    let mut child_stdin = child.stdin.take().expect("Failed to open stdin");
    let mut child_stdout = child.stdout.take().expect("Failed to open stdout");
    let mut child_stderr = child.stderr.take().expect("Failed to open stderr");

    let mut out = stdout();
    let mut err = stderr();
    
    tokio::select!{
        () = read_proc(&mut child_stdout, &mut out)=> {println!("Stdout closed")},
        () = read_proc(&mut child_stderr, &mut err) => {println!("Stderr closed")},
        ()= write_proc(&mut stream, &mut child_stdin) => {println!("Stdin closed")},
    }

    println!("Process exited");

    std::fs::remove_file(socket).expect("Failed to cleanup our socket when we finished with it");
}

async fn write_proc<T: AsyncWrite + Unpin>(p0: &mut UnixListener, output: &mut T) {
    loop {
        let (mut client, _) = p0.accept().await.expect("Failed to accept client");
        let mut buf = [0u8; 1024];
        match client.read(&mut buf).await {
            Ok(len) => {
                if len == 0 {
                    break;
                }
                output.write_all(&buf[..len]).await.expect("Failed to write");
            }
            Err(_) => {break}
        }
    }
}

async fn read_proc<T: AsyncRead + Unpin>(input: &mut T, output: &mut dyn Write) {
    loop {
        let mut buf = [0u8; 1024];
        match input.read(&mut buf).await {
            Ok(len) => {
                if len == 0 {
                    break;
                }
                output.write_all(&buf[..len]).expect("Failed to write");
            }
            Err(_) => {break}
        }
    }
}

fn print_help(exe: &str) {
    println!("{exe} has two modes, run mode and stdin mode.");
    println!();
    println!("Run mode: {exe} {{label}} {{program arguments}}");
    println!("Stdin mode: {exe} {{label}}");
    println!();
}

#[tokio::main]
async fn main() {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(_) => {
            let cmd = Args::command();
            let bin = cmd.get_bin_name().unwrap_or("pms");
            print_help(bin);
            println!("{}", Args::command().render_usage());
            exit(1);
        }
    };

    match args.cmd {
        Some(cmd) => {
            run_process(&args.tag, &cmd).await
        }
        None => {
            connect_process(&args.tag, &mut stdin()).await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{connect_process, run_process, socket_path};
    use rand::Rng;
    use serial_test::serial;
    use std::path::Path;
    use std::time;
    use tokio::join;
    use tokio::time::sleep;

    fn rand_label() -> String {
        let mut rng = rand::rng();
        let n1: u32 = rng.random();
        println!("Label: [{n1}]");
        format!("{n1}")
    }

    #[tokio::test]
    #[serial]
    #[should_panic]
    async fn test_no_process() {
        let label = rand_label();
        let socket = Path::new(&label);
        if socket.exists() {
            std::fs::remove_file(socket).expect("Failed to remove existing unix socket")
        }
        let cmd = String::from("ls\n");
        let mut stream = cmd.as_bytes();
        connect_process(&label, &mut stream).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_short_process() {
        let label = rand_label();

        run_process(&label, &[String::from("ls"), String::from("-l")]).await;

        let path = socket_path(&label);
        let socket = Path::new(&path);
        assert!(!socket.exists());
    }

    #[tokio::test]
    #[serial]
    async fn test_end_to_end() {
        let label = rand_label();

        async fn ls(label: &str) {
            sleep(time::Duration::from_secs(1)).await;
            let cmd = String::from("ls\n");
            let mut stream = cmd.as_bytes();
            connect_process(&label, &mut stream).await;
        }

        async fn exit(label: &str) {
            sleep(time::Duration::from_secs(2)).await;
            let cmd = String::from("exit\n");
            let mut stream = cmd.as_bytes();
            connect_process(&label, &mut stream).await;
        }

        let cmd = [String::from("bash"), String::from("-i")];

        join!(
            run_process(&label, &cmd),
            ls(&label),
            exit(&label)
        );
    }
}
