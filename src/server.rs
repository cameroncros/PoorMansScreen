use crate::errors::PMSServerError;
use crate::messages::proc_input::Input;
use crate::messages::proc_input::Input::Data;
use crate::messages::{ProcInput, ProcOutput};
use crate::socket_path;
use prost::Message;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::net::UnixListener;
use tokio::select;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub(crate) async fn run_process(label: &str, cmd: &[String]) -> Result<(), PMSServerError> {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());
    if socket.exists() {
        // std::fs::remove_file(socket).unwrap();
        return Err(PMSServerError::SocketAlreadyInUse);
    }
    let stream = UnixListener::bind(socket).map_err(PMSServerError::FailedToBind)?;
    let exe = cmd.first().unwrap();
    let args = &cmd[1..];
    let command = if args.is_empty() {
        pty_process::Command::new(exe)
    } else {
        pty_process::Command::new(exe).args(args)
    };
    let (mut pty, pts) = pty_process::open().map_err(PMSServerError::FailedCreatePTY)?;
    pty.resize(pty_process::Size::new(24, 80)).unwrap();

    let mut child = command
        .spawn(pts)
        .map_err(PMSServerError::FailedToSpawnChild)?;

    let (mut child_stdout, mut child_stdin) = pty.split();

    let (mut o_s, mut o_r) = unbounded_channel();
    let (mut i_s, mut i_r) = unbounded_channel();

    select! {
        e = handle_connections(&stream, &mut o_r, &mut i_s) => {println!("Connections broke - {e:#?}")}
        e = read_stdout(&mut child_stdout, &mut o_s)=> {println!("Stdout closed - {e:#?}")},
        e = write_stdin(&mut i_r, &mut child_stdin) => {println!("Stdin closed - {e:#?}")},
    }

    child
        .wait()
        .await
        .map_err(PMSServerError::FailedToWaitForChild)?;

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
        i_s.send(msg).map_err(PMSServerError::InputFailedToSend)?;
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
            stdout: buf[..len].to_vec(),
        };
        output
            .send(msg)
            .map_err(PMSServerError::OutputFailedToSend)?;
    }
    Err(PMSServerError::OutputFailedToRead)
}
