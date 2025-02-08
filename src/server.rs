use crate::errors::PMSServerError;
use crate::messages::proc_input::Input;
use crate::messages::proc_input::Input::Data;
use crate::messages::proc_output::Output;
use crate::messages::{ProcInput, ProcOutput};
use crate::socket_path;
use prost::Message;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::select;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub(crate) async fn run_process(label: &str, cmd: &[String]) -> Result<(), PMSServerError> {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());
    if socket.exists() {
        return Err(PMSServerError::SocketAlreadyInUse);
    }
    let stream = UnixListener::bind(socket).map_err(PMSServerError::FailedToBind)?;
    let exe = cmd.first().unwrap();
    let args = &cmd[1..];
    let mut command = Command::new(exe);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !args.is_empty() {
        command.args(args);
    }
    let mut child = command
        .spawn()
        .map_err(PMSServerError::FailedToSpawnChild)?;

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or(PMSServerError::FailedTakeStdinPipe)?;
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or(PMSServerError::FailedTakeStdoutPipe)?;
    let mut child_stderr = child
        .stderr
        .take()
        .ok_or(PMSServerError::FailedTakeStderrPipe)?;

    let (mut o_s, mut o_r) = unbounded_channel();
    let (mut i_s, mut i_r) = unbounded_channel();

    let mut o_s2 = o_s.clone();

    select! {
        e = handle_connections(&stream, &mut o_r, &mut i_s) => {println!("Connections broke - {e:#?}")}
        e = read_stdout(&mut child_stdout, &mut o_s)=> {println!("Stdout closed - {e:#?}")},
        e = read_stderr(&mut child_stderr, &mut o_s2) => {println!("Stderr closed - {e:#?}")},
        e = write_stdin(&mut i_r, &mut child_stdin) => {println!("Stdin closed - {e:#?}")},
    }

    std::fs::remove_file(socket).map_err(PMSServerError::FailedRemoveSocketFile)
}

async fn sender(
    o_r: &mut UnboundedReceiver<ProcOutput>,
    s: &mut WriteHalf<'_>,
) -> Result<(), PMSServerError> {
    while let Some(output) = o_r.recv().await {
        let bytes = output.encode_to_vec();
        s.write_u32(bytes.len() as u32)
            .await
            .map_err(PMSServerError::FailedWriteMsgLength)?;
        s.write_all(&bytes)
            .await
            .map_err(PMSServerError::FailedWriteMsg)?;
    }
    Ok(())
}

async fn receiver(
    r: &mut ReadHalf<'_>,
    i_s: &mut UnboundedSender<ProcInput>,
) -> Result<(), PMSServerError> {
    loop {
        let len = r
            .read_u32()
            .await
            .map_err(PMSServerError::FailedReadMsgLength)?;
        let mut buf = vec![0u8; len as usize];
        r.read_exact(&mut buf)
            .await
            .map_err(PMSServerError::FailedReadMsg)?;
        let msg = ProcInput::decode(&*buf).map_err(PMSServerError::InputDecodeError)?;
        i_s.send(msg)
            .map_err(|e| PMSServerError::InputFailedToSend(e))?;
    }
}

async fn handle_connections(
    stream: &UnixListener,
    o_r: &mut UnboundedReceiver<ProcOutput>,
    i_s: &mut UnboundedSender<ProcInput>,
) -> Result<(), PMSServerError> {
    loop {
        let (mut client, _) = stream
            .accept()
            .await
            .map_err(PMSServerError::FailedAcceptConnection)?;

        let (mut r, mut s) = client.split();

        select!(
            e = sender(o_r, &mut s) => {println!("Sender closed - {e:#?}")},
            e = receiver(&mut r, i_s) => {println!("Receiver closed - {e:#?}")},
        );
    }
}

async fn write_stdin<T: AsyncWrite + Unpin>(
    inputs: &mut UnboundedReceiver<ProcInput>,
    proc_stdin: &mut T,
) -> Result<(), PMSServerError> {
    while let Some(msg) = inputs.recv().await {
        match msg.input {
            None => {}
            Some(data) => match data {
                Data(data) => {
                    proc_stdin
                        .write_all(&data)
                        .await
                        .map_err(PMSServerError::InputFailedToWrite)?;
                }
                Input::Signal(_) => {
                    println!("Got signalled");
                    todo!();
                }
            },
        }
    }
    Ok(())
}

async fn read_stdout<T: AsyncRead + Unpin>(
    input: &mut T,
    output: &mut UnboundedSender<ProcOutput>,
) -> Result<(), PMSServerError> {
    let mut buf = vec![0; 1024];
    while let Ok(len) = input.read(&mut buf).await {
        if len == 0 {
            return Err(PMSServerError::InputInvalidLength);
        }
        let msg = ProcOutput {
            output: Some(Output::Stdout(buf[..len].to_vec())),
        };
        output
            .send(msg)
            .map_err(PMSServerError::OutputFailedToSend)?;
    }
    Err(PMSServerError::OutputFailedToRead)
}

async fn read_stderr<T: AsyncRead + Unpin>(
    input: &mut T,
    output: &mut UnboundedSender<ProcOutput>,
) -> Result<(), PMSServerError> {
    let mut buf = vec![0; 1024];
    while let Ok(len) = input.read(&mut buf).await {
        if len == 0 {
            return Err(PMSServerError::OutputInvalidLength);
        }
        let msg = ProcOutput {
            output: Some(Output::Stderr(buf[..len].to_vec())),
        };
        output
            .send(msg)
            .map_err(PMSServerError::OutputFailedToSend)?;
    }
    Err(PMSServerError::OutputFailedToRead)
}
