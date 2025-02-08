mod client;
mod errors;
mod server;

pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/pms.rs"));
}

use crate::client::connect_process;
use crate::server::run_process;
use clap::{CommandFactory, Parser};
use std::process::exit;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, trailing_var_arg = true)]
struct Args {
    /// The tag/label
    #[arg()]
    tag: String,

    /// The optional command
    #[arg()]
    cmd: Option<Vec<String>>,
}

fn socket_path(label: &str) -> String {
    format!("/tmp/{label}")
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
            // console_subscriber::init();
            run_process(&args.tag, &cmd).await.unwrap()
        }
        None => {
            console_subscriber::init();
            connect_process(&args.tag).await.unwrap()
        }
    }
    exit(0);
}

#[cfg(test)]
mod tests {
    use crate::client::connect_process;
    use crate::server::run_process;
    use crate::socket_path;
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
        connect_process(&label, &mut stream).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_short_process() {
        let label = rand_label();

        run_process(&label, &[String::from("ls"), String::from("-l")])
            .await
            .unwrap();

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
            connect_process(&label, &mut stream).await.unwrap();
        }

        async fn exit(label: &str) {
            sleep(time::Duration::from_secs(2)).await;
            let cmd = String::from("exit\n");
            let mut stream = cmd.as_bytes();
            connect_process(&label, &mut stream).await.unwrap();
        }

        let cmd = [String::from("bash"), String::from("-i")];

        let (_, (), ()) = join!(run_process(&label, &cmd), ls(&label), exit(&label));
    }
}
