use crate::errors::PMSClientError;
use crate::messages::proc_input::Input;
use crate::messages::proc_input::Input::Data;
use crate::messages::{ProcInput, ProcOutput, Size};
use crate::socket_path;
use crossterm::terminal::size;
use prost::Message;
use std::io;
use std::path::Path;
use tokio::io::{stdout, AsyncRead};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::net::UnixStream;
use tokio::select;

pub(crate) async fn connect_process<T: AsyncRead + Unpin>(
    label: &str,
    input: &mut T,
) -> Result<(), PMSClientError> {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());

    let mut stream = UnixStream::connect(socket)
        .await
        .map_err(PMSClientError::FailedConnect)?;

    let (mut r, mut w) = stream.split();
    select!(
        e = handle_stdout(&mut r) => {
            if e.is_err() {
                println!("Stdout/err failed - {e:#?}");
            }
        },
        e = handle_stdin(&mut w, input) => {
            if e.is_err() {
                println!("Stdin failed - {e:#?}")
            }
        }
    );

    Ok(())
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

async fn send_msg<T: AsyncReadExt + Unpin>(
    sock: &mut WriteHalf<'_>,
    msg: &ProcInput,
) -> Result<(), PMSClientError> {
    sock.write_u32(msg.encoded_len() as u32)
        .await
        .map_err(PMSClientError::FailedWriteMsgLength)?;
    sock.write_all(msg.encode_to_vec().as_slice())
        .await
        .map_err(PMSClientError::FailedWriteMsg)
}

async fn handle_stdin<T: AsyncReadExt + Unpin>(
    sock: &mut WriteHalf<'_>,
    input: &mut T,
) -> Result<(), PMSClientError> {
    if let Ok((w, h)) = size() {
        let msg = ProcInput {
            input: Some(Input::Size(Size {
                w: w as u32,
                h: h as u32,
            })),
        };
        send_msg::<T>(sock, &msg).await?;
    }
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
        if len == 1 && buf[0] == 0x03 {
            return Ok(());
        }
        let msg = ProcInput {
            input: Some(Data(buf[..len].to_vec())),
        };
        send_msg::<T>(sock, &msg).await?;
    }
}
