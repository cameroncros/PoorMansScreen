use std::{env, io, thread};
use std::io::{Read, stderr, stdin, stdout, Write};
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::process::{Command, exit, Stdio};

struct RWUnixDatagram {
    unixdatagram: UnixDatagram
}

impl Read for RWUnixDatagram {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.unixdatagram.recv(buf)
    }
}

impl Write for RWUnixDatagram {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.unixdatagram.send(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl RWUnixDatagram {
    pub fn unbound() -> io::Result<RWUnixDatagram> {
        Ok(RWUnixDatagram {
            unixdatagram: UnixDatagram::unbound()?
        })
    }
    
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<RWUnixDatagram> {
        match UnixDatagram::bind(path) {
            Ok(ud) => {
                Ok(RWUnixDatagram { unixdatagram: ud })
            }
            Err(e) => { Err(e) }
        }
    }

    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.unixdatagram.connect(path)
    }
}


fn socket_path(label: &str) -> String
{
    format!("/tmp/{label}")
}

fn connect_process(label: &str, stdin: &mut dyn Read) -> io::Result<()>
{
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());

    // Connect to socket
    let mut sock = RWUnixDatagram::unbound().expect("Failed to create unix socket");
    sock.connect(socket).expect("Failed to connect to unix socket");

    std::io::copy(stdin, &mut sock).expect("Failed to write to socket");
    Ok(())
}

fn run_process(label: &str, cmd: &[String]) -> io::Result<()>{
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
    let exe = args.first().unwrap();

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
        assert!(Path::new(&path).exists());
    }
}
