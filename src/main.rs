mod client;
mod errors;
mod server;

pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/pms.rs"));
}

use crate::client::connect_process;
use crate::server::run_process;
use clap::{CommandFactory, Parser};
use std::env;
use std::path::Path;
use std::process::{exit, Command};
use std::time::Duration;
use tokio::io::stdin;
use tokio::time::sleep;

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

async fn fork_and_run(args: Args) {
    let stage: u32 = env::var("PMS_STAGE")
        .unwrap_or(String::from("0"))
        .parse::<u32>()
        .unwrap();
    let exe = env::args().next().unwrap();
    let child_cmd = args.cmd.unwrap();
    let next_stage = (stage + 1).to_string();
    let tag = args.tag.clone();

    let mut next_args = vec![tag];
    next_args.extend(child_cmd.clone());

    let mut cmd = Command::new(exe);
    cmd.args(&next_args);
    cmd.env("PMS_STAGE", next_stage);

    if stage == 0 {
        let _ = cmd.spawn().unwrap().wait().unwrap();
    } else if stage == 1 {
        let _ = cmd.spawn().unwrap();
        exit(0);
    } else if stage == 2 {
        run_process(&args.tag, &child_cmd).await.unwrap();
        exit(0);
    }
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

    let tag = args.tag.clone();

    if args.cmd.is_some() {
        fork_and_run(args).await;
        let s_path = socket_path(&tag);
        let s = Path::new(&s_path);
        for _ in 0..5 {
            if s.exists() {
                break;
            }
            sleep(Duration::from_millis(1000)).await;
        }
        if !s.exists() {
            println!("Child process didnt spawn?");
            exit(1);
        }
    }

    // console_subscriber::init();

    connect_process(&tag, &mut stdin()).await.unwrap();

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
