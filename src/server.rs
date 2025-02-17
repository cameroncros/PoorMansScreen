use crate::errors::PMSServerError;
use crate::messages::proc_input::Input;
use crate::messages::proc_input::Input::{Data, Size};
use crate::messages::{ProcInput, ProcOutput};
use crate::socket_path;
use prost::Message;
use pty_process::{ReadPty, WritePty};
use ring_channel::{ring_channel, RingReceiver, RingSender, TryRecvError};
use std::num::NonZero;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::net::UnixListener;
use tokio::select;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::yield_now;
use tracing::{debug, trace};

pub(crate) async fn run_process(label: &str, cmd: &[String]) -> Result<(), PMSServerError> {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());
    if socket.exists() {
        return Err(PMSServerError::SocketAlreadyInUse);
    }
    let stream = UnixListener::bind(socket).map_err(PMSServerError::FailedToBind)?;
    let Some(exe) = cmd.first() else {
        return Err(PMSServerError::InvalidExe);
    };
    let args = &cmd[1..];
    let command = if args.is_empty() {
        pty_process::Command::new(exe)
    } else {
        pty_process::Command::new(exe).args(args)
    };
    let (mut pty, pts) = pty_process::open().map_err(PMSServerError::FailedCreatePTY)?;
    pty.resize(pty_process::Size::new(24, 80))
        .map_err(PMSServerError::FailedToResize)?;

    let mut child = command
        .spawn(pts)
        .map_err(PMSServerError::FailedToSpawnChild)?;

    let (mut child_stdout, mut child_stdin) = pty.split();

    let (mut o_s, mut o_r) = ring_channel::<ProcOutput>(NonZero::new(100000).unwrap());
    let (mut i_s, mut i_r) = unbounded_channel();

    select! {
        e = handle_connections(&stream, &mut o_r, &mut i_s) => {
            if e.is_err() {
                debug!("Connections broke - {e:#?}");
            }
        },
        e = read_stdout(&mut child_stdout, &mut o_s)=> {
            if e.is_err() {
                debug!("Stdout closed - {e:#?}");
            }
        },
        e = write_stdin(&mut i_r, &mut child_stdin) => {
            if e.is_err() {
                debug!("Stdin closed - {e:#?}");
            }
        },
    }

    if let Err(e) = child.wait().await {
        debug!("Child exited - closed - {e:#?}");
    };

    std::fs::remove_file(socket).map_err(PMSServerError::FailedRemoveSocketFile)
}

async fn sender(
    o_r: &mut RingReceiver<ProcOutput>,
    s: &mut WriteHalf<'_>,
) -> Result<(), PMSServerError> {
    loop {
        let output = match o_r.try_recv() {
            Ok(m) => m,
            Err(e) => match e {
                TryRecvError::Empty => {
                    yield_now().await;
                    continue;
                }
                TryRecvError::Disconnected => {
                    debug!("Sender closed - disconnected");
                    return Ok(());
                }
            },
        };
        let bytes = output.encode_to_vec();
        s.write_u32(bytes.len() as u32)
            .await
            .map_err(PMSServerError::FailedWriteMsgLength)?;
        s.write_all(&bytes)
            .await
            .map_err(PMSServerError::FailedWriteMsg)?;
    }
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
    o_r: &mut RingReceiver<ProcOutput>,
    i_s: &mut UnboundedSender<ProcInput>,
) -> Result<(), PMSServerError> {
    loop {
        debug!("Waiting for connection");
        let (mut client, _) = match stream.accept().await {
            Ok(v) => v,
            Err(e) => {
                debug!("Failed to accept connection: {e:#?}");
                continue;
            }
        };

        debug!("Waiting for connection");
        let (mut r, mut s) = client.split();

        select!(
            e = sender(o_r, &mut s) => {debug!("Sender closed - {e:#?}")},
            e = receiver(&mut r, i_s) => {debug!("Receiver closed - {e:#?}")},
        );
    }
}

async fn write_stdin(
    inputs: &mut UnboundedReceiver<ProcInput>,
    proc_stdin: &mut WritePty<'_>,
) -> Result<(), PMSServerError> {
    while let Some(msg) = inputs.recv().await {
        match msg.input {
            None => {}
            Some(data) => match data {
                Data(data) => {
                    trace!("Input: [{:#?}]", data);
                    proc_stdin
                        .write_all(&data)
                        .await
                        .map_err(PMSServerError::InputFailedToWrite)?;
                }
                Size(size) => {
                    proc_stdin
                        .resize(pty_process::Size::new(size.h as u16, size.w as u16))
                        .map_err(PMSServerError::FailedToResize)?;
                }
                Input::Signal(s) => {
                    debug!("Got signalled - {s}");
                    // let sig =
                    //     Signal::try_from(s as i32).map_err(PMSServerError::FailedToParseSignal)?;
                    // let cpid = Pid::from_raw(child_pid as pid_t);
                    // let pgid = getpgid(cpid).unwrap();
                    // killpg(pgid, sig).map_err(PMSServerError::FailedToSignalChild)?;
                }
            },
        }
    }
    Ok(())
}

async fn read_stdout(
    input: &mut ReadPty<'_>,
    output: &mut RingSender<ProcOutput>,
) -> Result<(), PMSServerError> {
    let mut buf = vec![0; 1024];
    while let Ok(len) = input.read(&mut buf).await {
        if len == 0 {
            return Ok(());
        }
        let msg = ProcOutput {
            stdout: buf[..len].to_vec(),
        };
        trace!("Output: [{:#?}]", buf[..len].to_vec());
        output
            .send(msg)
            .map_err(PMSServerError::OutputFailedToSend)?;
    }
    Err(PMSServerError::OutputFailedToRead)
}
