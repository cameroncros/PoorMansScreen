use crate::errors::PMSClientError;
use crate::messages::proc_input::Input;
use crate::messages::proc_input::Input::Data;
use crate::messages::{ProcInput, ProcOutput, Size};
use crate::socket_path;
use crossterm::terminal::size;
use futures_util::stream::StreamExt;
use prost::Message;
use std::io;
use std::path::Path;
use tokio::io::{stdout, AsyncRead};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::net::UnixStream;
use tokio::select;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;

pub(crate) async fn connect_process<T: AsyncRead + Unpin>(
    label: &str,
    input: &mut T,
) -> Result<(), PMSClientError> {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());

    let mut stream = UnixStream::connect(socket)
        .await
        .map_err(PMSClientError::FailedConnect)?;

    let signals = Signals::new(vec![SIGWINCH, SIGINT, SIGTERM])
        .map_err(PMSClientError::FailedSignalHandler)?;
    let handle = signals.handle();

    let (mut r, mut w) = stream.split();

    let (channel_in, mut channel_out) = unbounded_channel();
    let channel_in2 = channel_in.clone();

    select!(
        e = handle_stdout(&mut r) => {
            if e.is_err() {
                println!("Stdout/err failed - {e:#?}");
            }
        },
        e = handle_stdin(input, &channel_in) => {
            if e.is_err() {
                println!("Stdin failed - {e:#?}")
            }
        },
        e = handle_signals(signals, &channel_in2) => {
            if e.is_err() {
                println!("Stdin failed - {e:#?}")
            }
        },
        e = handle_send(&mut channel_out, &mut w) => {
            if e.is_err() {
                println!("Sender failed - {e:#?}");
            }
        },
    );

    handle.close();

    Ok(())
}

async fn handle_send(
    co: &mut UnboundedReceiver<ProcInput>,
    sock: &mut WriteHalf<'_>,
) -> Result<(), PMSClientError> {
    loop {
        match co.recv().await {
            None => continue,
            Some(msg) => {
                sock.write_u32(msg.encoded_len() as u32)
                    .await
                    .map_err(PMSClientError::FailedWriteMsgLength)?;
                sock.write_all(msg.encode_to_vec().as_slice())
                    .await
                    .map_err(PMSClientError::FailedWriteMsg)?;
            }
        }
    }
}

async fn send_size(send_channel: &UnboundedSender<ProcInput>) -> Result<(), PMSClientError> {
    if let Ok((w, h)) = size() {
        let msg = ProcInput {
            input: Some(Input::Size(Size {
                w: w as u32,
                h: h as u32,
            })),
        };
        send_channel
            .send(msg)
            .map_err(PMSClientError::FailedQueueMsg)?;
    }
    Ok(())
}

async fn send_signal(
    send_channel: &UnboundedSender<ProcInput>,
    signal: u32,
) -> Result<(), PMSClientError> {
    let msg = ProcInput {
        input: Some(Input::Signal(signal)),
    };
    send_channel
        .send(msg)
        .map_err(PMSClientError::FailedQueueMsg)
}

async fn send_data(
    send_channel: &UnboundedSender<ProcInput>,
    data: &[u8],
) -> Result<(), PMSClientError> {
    let msg = ProcInput {
        input: Some(Data(data.to_vec())),
    };
    send_channel
        .send(msg)
        .map_err(PMSClientError::FailedQueueMsg)
}

async fn handle_signals(
    mut signals: Signals,
    send_channel: &UnboundedSender<ProcInput>,
) -> Result<(), PMSClientError> {
    send_size(send_channel).await?;

    while let Some(signal) = signals.next().await {
        match signal {
            SIGWINCH => {
                send_size(send_channel).await?;
            }
            SIGINT => {
                send_signal(send_channel, SIGINT as u32).await?;
                send_data(send_channel, &[0x03u8]).await?;
            }
            SIGTERM => {
                send_signal(send_channel, SIGTERM as u32).await?;
            }
            _ => unreachable!(),
        }
    }
    unreachable!()
}

async fn handle_stdout(r: &mut ReadHalf<'_>) -> Result<(), PMSClientError> {
    let mut output = stdout();

    loop {
        let Ok(len) = r.read_u32().await else {
            return Ok(());
        };
        let mut buf = vec![0; len as usize];
        r.read_exact(&mut buf)
            .await
            .map_err(PMSClientError::FailedReadMsg)?;

        let msg = ProcOutput::decode(&*buf).map_err(PMSClientError::OutputFailedToDecode)?;

        output
            .write_all(&msg.stdout)
            .await
            .map_err(PMSClientError::FailedWriteStdout)?;

        output.flush().await.unwrap();
    }
}

async fn handle_stdin<T: AsyncReadExt + Unpin>(
    input: &mut T,
    send_channel: &UnboundedSender<ProcInput>,
) -> Result<(), PMSClientError> {
    let mut sup = false;
    loop {
        let mut buf = [0; 1024];
        let len = match input.read(&mut buf).await {
            Ok(len) => len,
            Err(e) => match e.kind() {
                io::ErrorKind::Interrupted => {
                    continue;
                }
                _ => return Err(PMSClientError::FailedReadStdin(e)),
            },
        };
        if len == 0 {
            return Ok(());
        }
        if len == 1 {
            match buf[0] {
                0x01 => {
                    // Ctrl-A
                    sup = true;
                    continue;
                }
                0x03 => {
                    if !sup {
                        send_signal(send_channel, SIGINT as u32).await?;
                        send_data(send_channel, &[0x03u8]).await?;
                        continue;
                    } else {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        sup = false;

        send_data(send_channel, &buf[..len]).await?;
    }
}
