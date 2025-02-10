use crate::messages::{ProcInput, ProcOutput};
use prost::DecodeError;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

#[derive(Error, Debug)]
pub enum PMSClientError {
    #[error("TODO")]
    OutputFailedToDecode(DecodeError),
    #[error("TODO")]
    FailedReadMsg(std::io::Error),
    #[error("TODO")]
    FailedReadStdin(std::io::Error),
    #[error("TODO")]
    FailedWriteMsgLength(std::io::Error),
    #[error("TODO")]
    FailedWriteMsg(std::io::Error),
    #[error("TODO")]
    FailedWriteStdout(std::io::Error),
    #[error("TODO")]
    FailedConnect(std::io::Error),
}

#[derive(Error, Debug)]
pub enum PMSServerError {
    #[error("TODO")]
    SocketAlreadyInUse,
    #[error("TODO")]
    FailedToSpawnChild(pty_process::Error),
    #[error("TODO")]
    FailedToBind(std::io::Error),
    #[error("TODO")]
    FailedRemoveSocketFile(std::io::Error),
    #[error("TODO")]
    FailedWriteMsgLength(std::io::Error),
    #[error("TODO")]
    FailedWriteMsg(std::io::Error),
    #[error("TODO")]
    FailedReadMsgLength(std::io::Error),
    #[error("TODO")]
    FailedReadMsg(std::io::Error),
    #[error("TODO")]
    InputDecodeError(DecodeError),
    #[error("TODO")]
    InputFailedToSend(#[from] SendError<ProcInput>),
    #[error("TODO")]
    InputFailedToWrite(std::io::Error),
    #[error("TODO")]
    OutputFailedToSend(SendError<ProcOutput>),
    #[error("TODO")]
    OutputFailedToRead,
    #[error("TODO")]
    FailedAcceptConnection(std::io::Error),
    #[error("TODO")]
    FailedCreatePTY(pty_process::Error),
    #[error("TODO")]
    FailedToWaitForChild(std::io::Error),
}
