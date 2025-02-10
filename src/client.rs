use crate::errors::PMSClientError;
use crate::messages::proc_input::Input::Data;
use crate::messages::{ProcInput, ProcOutput};
use crate::socket_path;
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
    loop {
        let Ok(len) = r.read_u32().await else {
            return Ok(());
        };
        let mut buf = vec![0; len as usize];
        r.read_exact(&mut buf)
            .await
            .map_err(PMSClientError::FailedReadMsg)?;

        let msg = ProcOutput::decode(&*buf).map_err(PMSClientError::OutputFailedToDecode)?;
        stdout()
            .write_all(&msg.stdout)
            .await
            .map_err(PMSClientError::FailedWriteStdout)?;
    }
}

async fn handle_stdin<T: AsyncRead + Unpin>(
    sock: &mut WriteHalf<'_>,
    input: &mut T,
) -> Result<(), PMSClientError> {
    loop {
        let mut buf = vec![0; 1024];
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
            continue;
        }
        let msg = ProcInput {
            input: Some(Data(buf[..len].to_vec())),
        };
        sock.write_u32(msg.encoded_len() as u32)
            .await
            .map_err(PMSClientError::FailedWriteMsgLength)?;
        sock.write_all(msg.encode_to_vec().as_slice())
            .await
            .map_err(PMSClientError::FailedWriteMsg)?;
    }
}
