use clap::{CommandFactory, Parser};
use rwunixdatagram::RWUnixDatagram;
use std::io::{stderr, stdin, stdout, Read};
use std::path::Path;
use std::process::{exit, Command, Stdio};
use std::thread;

mod rwunixdatagram;

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

fn connect_process(label: &str, stdin: &mut dyn Read)
{
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());

    // Connect to socket
    let mut sock = RWUnixDatagram::unbound().expect("Failed to create unix socket");
    sock.connect(socket).expect("Failed to connect to unix socket");

    std::io::copy(stdin, &mut sock).expect("Failed to write to socket");
}

fn run_process(label: &str, cmd: &[String]) {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());
    if socket.exists() {
        std::fs::remove_file(socket).expect("Failed to remove existing unix socket")
    }
    let mut stream = match RWUnixDatagram::bind(socket) {
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
    
    let thread_out = thread::spawn(move || {
        std::io::copy(&mut child_stdout, &mut stdout()).unwrap();
    });
    let thread_err = thread::spawn(move || {
        std::io::copy(&mut child_stderr, &mut stderr()).unwrap();
    });
    thread::spawn(move || {
        std::io::copy(&mut stream, &mut child_stdin).unwrap();
    });
    thread_out.join().unwrap();
    thread_err.join().unwrap();
    // Don't bother closing stdin thread, just exit.
    std::fs::remove_file(socket).expect("Failed to cleanup our socket when we finished with it");
}

fn print_help(exe: &str) {
    println!("{exe} has two modes, run mode and stdin mode.");
    println!();
    println!("Run mode: {exe} {{label}} {{program arguments}}");
    println!("Stdin mode: {exe} {{label}}");
    println!();
}

fn main() {
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
            run_process(&args.tag, &cmd)
        }
        None => {
            connect_process(&args.tag, &mut stdin())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{connect_process, run_process, socket_path};
    use fork::fork;
    use fork::Fork::{Child, Parent};
    use nix::sys::wait::waitpid;
    use nix::unistd::Pid;
    use rand::Rng;
    use serial_test::serial;
    use std::path::Path;
    use std::{thread, time};

    fn rand_label() -> String {
        let mut rng = rand::thread_rng();
        let n1: u32 = rng.gen();
        println!("Label: [{n1}]");
        format!("{n1}")
    }

    #[test]
    #[serial]
    #[should_panic]
    fn test_no_process() {
        let label = rand_label();
        let socket = Path::new(&label);
        if socket.exists() {
            std::fs::remove_file(socket).expect("Failed to remove existing unix socket")
        }
        let cmd = String::from("ls\n");
        let mut stream = cmd.as_bytes();
        connect_process(&label, &mut stream);
    }

    #[test]
    #[serial]
    fn test_short_process() {
        let label = rand_label();
        
        run_process(&label, &[String::from("ls"), String::from("-l")]);

        let path = socket_path(&label);
        let socket = Path::new(&path);
        assert_eq!(false, socket.exists());
    }

    #[test]
    #[serial]
    fn test_end_to_end() {
        let label = rand_label();
        match fork().expect("Failed to fork") {
            Child => {
                run_process(&label, &[String::from("/bin/bash"), String::from("-i")])
            }
            Parent(pid) => {
                println!("PID: {pid}");
                let path = socket_path(&label);
                let socket = Path::new(&path);
                loop {
                    if socket.exists() {
                        break;
                    }
                    thread::sleep(time::Duration::from_millis(100));
                }
                {
                    let cmd = String::from("ls\n");
                    let mut stream = cmd.as_bytes();
                    connect_process(&label, &mut stream);
                }
                {
                    let cmd = String::from("exit\n");
                    let mut stream = cmd.as_bytes();
                    connect_process(&label, &mut stream);
                }
                println!("Waiting for process");
                waitpid(Option::from(Pid::from_raw(pid)), None).unwrap();
            }
        }
        let path = socket_path(&label);
        assert!(!Path::new(&path).exists());
    }
}
