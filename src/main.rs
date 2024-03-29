use std::{env, thread};
use std::io::{Read, stderr, stdin, stdout, Write};
use std::os::unix::net::{UnixDatagram};
use std::path::Path;
use std::process::{Command, exit, Stdio};

struct ReadableUnixDatagram {
    unixdatagram: UnixDatagram
}

impl Read for ReadableUnixDatagram {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        return self.unixdatagram.recv(buf)
    }
}

impl ReadableUnixDatagram {
    pub fn bind<P: AsRef<Path>>(path: P) -> std::io::Result<ReadableUnixDatagram> {
        match UnixDatagram::bind(path) {
            Ok(ud) => {
                let rud = ReadableUnixDatagram { unixdatagram: ud };
                return Ok(rud)
            }
            Err(e) => {return Err(e)}
        }
    }
}

fn socket_path(label: &String) -> String
{
    return format!("/tmp/{label}")
}
fn connect_process(label: &String, stdin: &mut dyn Read) -> Result<(), String>
{
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());

    // Connect to socket
    let sock = UnixDatagram::unbound().expect("Failed to create unix socket");
    sock.connect(&socket).expect("Failed to connect to unix socket");

    loop
    {
        let buffer : &mut [u8;10000] = &mut [0u8;10000];
        let size = stdin.read(buffer).expect("Failed to read from stdin");
        if size == 0 {
            break;
        }
        sock.send(&buffer[0..size]).expect("Failed to send to unix socket");
    }
    Ok(())
}

fn run_process(label: &String, cmd: &[String]) -> Result<(), ()> {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());
    if socket.exists() {
        std::fs::remove_file(socket).expect("Failed to remove existing unix socket")
    }
    let stream = match ReadableUnixDatagram::bind(&socket) {
        Err(_) => panic!("Failed to create unix socket"),
        Ok(stream) => stream,
    };

    let exe = cmd.get(0).unwrap();
    let args = &cmd[1..];
    let mut command = Command::new(exe);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if args.len() != 0 {
        command.args(args);
    }
    let mut child = command.spawn().expect("Failed to start child process");

    let child_stdin = child.stdin.take().expect("Failed to open stdin");
    let child_stdout = child.stdout.take().expect("Failed to open stdout");
    let child_stderr = child.stderr.take().expect("Failed to open stderr");

    fn communicate(
        mut stream: impl Read,
        mut output: impl Write,
    ) {
        let mut buf = [0u8; 1024];
        loop {
            let num_read = stream.read(&mut buf).expect("Failed to read from unix socket");
            if num_read == 0 {
                break;
            }
            output.write_all(&buf[0..num_read]).expect("Failed to write to process");
        }
    }
    let thread_out = thread::spawn(move || {
        communicate(child_stdout, stdout())
    });
    let thread_err = thread::spawn(move || {
        communicate(child_stderr, stderr())
    });
    thread::spawn(move || {
        communicate(stream, child_stdin)
    });
    thread_out.join().unwrap();
    thread_err.join().unwrap();
    // Dont bother closing stdin thread, just exit.
    std::fs::remove_file(socket).expect("Failed to cleanup our socket when we finished with it");
    exit(0);
}

fn print_help(exe: &String) {
    println!("{exe} has two modes, run mode and stdin mode.");
    println!();
    println!("Run mode: {exe} {{label}} {{program arguments}}");
    println!("Stdin mode: {exe} {{label}}");
    exit(0);
}

fn main() {
    let n_args = env::args().count();
    let args: Vec<String> = env::args().map(|x| x.to_string())
        .collect();
    let exe = args.get(0).unwrap();

    if n_args < 2 {
        print_help(exe);
    }
    let label = args.get(1).unwrap();
    if n_args == 2 {
        connect_process(label, &mut stdin()).unwrap();
        exit(0);
    }
    if n_args >= 3 {
        run_process(label, &args[2..]).unwrap();
        exit(0);
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time};
    use std::path::Path;
    use fork::fork;
    use fork::Fork::{Child, Parent};
    use nix::sys::wait::waitpid;
    use nix::unistd::Pid;
    use crate::{connect_process, run_process, socket_path};
    use rand::Rng;
    use serial_test::serial;

    fn rand_label() -> String {
        let mut rng = rand::thread_rng();
        let n1: u32 = rng.gen();
        println!("Label: [{n1}]");
        return format!("{n1}");
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
        connect_process(&label, &mut stream).unwrap();
    }

    #[test]
    #[serial]
    fn test_end_to_end() {
        let label = rand_label();
        match fork().expect("Failed to fork") {
            Child => {
                run_process(&label, &[String::from("/bin/bash"), String::from("-i")]).unwrap()
            }
            Parent(pid) => {
                thread::sleep(time::Duration::from_secs(1));
                {
                    let cmd = String::from("ls\n");
                    let mut stream = cmd.as_bytes();
                    connect_process(&label, &mut stream).unwrap();
                }
                {
                    let cmd = String::from("exit\n");
                    let mut stream = cmd.as_bytes();
                    connect_process(&label, &mut stream).unwrap();
                }
                println!("Waiting for process");
                waitpid(Option::from(Pid::from_raw(pid)), None).unwrap();
            }
        }
        let path = socket_path(&label);
        assert_eq!(false, Path::new(&path).exists());
    }
}
