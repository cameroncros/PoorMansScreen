use crate::errors::PMSClientError;
use crate::messages::proc_input::Input::Data;
use crate::messages::proc_output::Output;
use crate::messages::{ProcInput, ProcOutput};
use crate::socket_path;
use prost::Message;
use std::path::Path;
use tokio::io::{stderr, stdin, stdout};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::unix::{ReadHalf, WriteHalf};
use tokio::net::UnixStream;
use tokio::select;

pub(crate) async fn connect_process(label: &str) -> Result<(), PMSClientError> {
    let socket_path = socket_path(label);
    let socket = Path::new(socket_path.as_str());

    let mut stream = UnixStream::connect(socket)
        .await
        .map_err(PMSClientError::FailedConnect)?;

    let (mut r, mut w) = stream.split();
    select!(
        e = handle_stdout(&mut r) => {println!("Stdout/err failed - {e:#?}")},
        e = handle_stdin(&mut w) => {println!("Stdin failed - {e:#?}")},
    );

    Ok(())
}

async fn handle_stdout(r: &mut ReadHalf<'_>) -> Result<(), PMSClientError> {
    loop {
        let len = r
            .read_u32()
            .await
            .map_err(PMSClientError::FailedReadMsgLength)?;
        let mut buf = vec![0; len as usize];
        r.read_exact(&mut buf)
            .await
            .map_err(PMSClientError::FailedReadMsg)?;

        let msg = ProcOutput::decode(&*buf).map_err(PMSClientError::OutputFailedToDecode)?;
        match msg.output {
            None => {}
            Some(out) => match out {
                Output::Stdout(data) => stdout()
                    .write_all(&data)
                    .await
                    .map_err(PMSClientError::FailedWriteStdout)?,
                Output::Stderr(data) => stderr()
                    .write_all(&data)
                    .await
                    .map_err(PMSClientError::FailedWriteStderr)?,
            },
        }
    }
}

async fn handle_stdin(sock: &mut WriteHalf<'_>) -> Result<(), PMSClientError> {
    loop {
        let mut buf = vec![0; 1024];
        let len = stdin()
            .read(&mut buf)
            .await
            .map_err(PMSClientError::FailedReadStdin)?;
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
