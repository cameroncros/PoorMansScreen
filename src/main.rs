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
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{exit, Command};
use std::time::Duration;
use tokio::io::stdin;
use tokio::time::sleep;
use tracing::metadata::LevelFilter;
use tracing::{debug, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, Layer, Registry};

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

async fn wait_for_socket(tag: &str, timeout: u32) {
    let s_path = socket_path(tag);
    let s = Path::new(&s_path);
    for _ in 0..timeout * 100 {
        if s.exists() {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    if !s.exists() {
        println!("Child process didnt spawn?");
        exit(1);
    }
}

async fn fork_and_run(cmdline: &[String], args: &Args) {
    let stage: u32 = env::var("PMS_STAGE")
        .unwrap_or(String::from("0"))
        .parse::<u32>()
        .unwrap();
    let next_stage = (stage + 1).to_string();

    let mut cmd = Command::new(&cmdline[0]);
    cmd.args(&cmdline[1..]);
    cmd.env("PMS_STAGE", next_stage);

    if stage == 0 {
        debug!("Spawned stage 1");
        let _ = cmd.spawn().unwrap().wait().unwrap();
        debug!("Stage 1 exited");
        wait_for_socket(&args.tag.clone(), 5).await;
    } else if stage == 1 {
        let _ = cmd.spawn().unwrap();
        debug!("Spawned stage 2, exiting");
        exit(0);
    } else if stage == 2 {
        debug!("Executing command");
        if let Err(e) = run_process(&args.tag, &args.cmd.clone().unwrap()).await {
            debug!("Process exited with error: {}", e);
        };
        debug!("Process exited");
        exit(0);
    }
}

fn setup_logging() {
    let debug_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("/tmp/pms-debug.log")
        .unwrap();
    let subscriber = Registry::default().with(
        // log-debug file, to log the debug
        fmt::layer()
            .with_writer(debug_file)
            .with_filter(LevelFilter::from(Level::DEBUG)),
    );

    tracing::subscriber::set_global_default(subscriber).unwrap();
}

#[tokio::main]
async fn main() {
    if env::var("PMS_LOGGING").is_ok() {
        setup_logging();
    }

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
        let cmdline = env::args().collect::<Vec<String>>();
        fork_and_run(&cmdline, &args).await;
        // run_process(&tag, &args.cmd.unwrap()).await.unwrap();
    }

    if let Err(e) = crossterm::terminal::enable_raw_mode() {
        println!("Failed to enable raw mode: {e}");
        exit(1);
    }

    // console_subscriber::init();

    if let Err(e) = connect_process(&tag, &mut stdin()).await {
        println!("Connection failed: {e}");
    };

    crossterm::terminal::disable_raw_mode().unwrap();
}

#[cfg(test)]
mod tests {
    use crate::client::connect_process;
    use crate::server::run_process;
    use crate::{fork_and_run, socket_path, Args};
    use rand::Rng;
    use serial_test::serial;
    use std::path::Path;
    use std::time;
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

        let cmdline = vec![
            String::from("cargo"),
            String::from("run"),
            label.clone(),
            String::from("bash"),
            String::from("-i"),
        ];

        let args = Args {
            tag: label.clone(),
            cmd: None,
        };

        fork_and_run(&cmdline, &args).await;

        sleep(time::Duration::from_secs(1)).await;
        let cmd = String::from("ls\n");
        let mut stream = cmd.as_bytes();
        connect_process(&label, &mut stream).await.unwrap();

        sleep(time::Duration::from_secs(1)).await;
        let cmd = String::from("exit\n");
        let mut stream = cmd.as_bytes();
        connect_process(&label, &mut stream).await.unwrap();
    }
}
