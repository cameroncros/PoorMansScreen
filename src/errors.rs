use crate::messages::{ProcInput, ProcOutput};
use prost::DecodeError;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

#[derive(Error, Debug)]
pub enum PMSClientError {
    #[error("TODO")]
    OutputFailedToDecode(DecodeError),
    #[error("TODO")]
    FailedReadMsgLength(std::io::Error),
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
    FailedWriteStderr(std::io::Error),
    #[error("TODO")]
    FailedConnect(std::io::Error),
}

#[derive(Error, Debug)]
pub enum PMSServerError {
    #[error("TODO")]
    SocketAlreadyInUse,
    #[error("TODO")]
    FailedToSpawnChild(std::io::Error),
    #[error("TODO")]
    FailedToBind(std::io::Error),
    #[error("TODO")]
    FailedTakeStdinPipe,
    #[error("TODO")]
    FailedTakeStdoutPipe,
    #[error("TODO")]
    FailedTakeStderrPipe,
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
    InputInvalidLength,
    #[error("TODO")]
    OutputFailedToSend(SendError<ProcOutput>),
    #[error("TODO")]
    OutputFailedToRead,
    #[error("TODO")]
    OutputInvalidLength,
    #[error("TODO")]
    FailedAcceptConnection(std::io::Error),
}
