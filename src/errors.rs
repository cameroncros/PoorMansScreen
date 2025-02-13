use crate::messages::{ProcInput, ProcOutput};
use prost::DecodeError;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

#[derive(Error, Debug)]
pub enum PMSClientError {
    #[error("Failed to decode message - {0}")]
    OutputFailedToDecode(DecodeError),
    #[error("Failed to receive message - {0}")]
    FailedReadMsg(std::io::Error),
    #[error("Failed to read stdin - {0}")]
    FailedReadStdin(std::io::Error),
    #[error("Failed to send msg length - {0}")]
    FailedWriteMsgLength(std::io::Error),
    #[error("Failed to send msg - {0}")]
    FailedWriteMsg(std::io::Error),
    #[error("Failed to print to stdout - {0}")]
    FailedWriteStdout(std::io::Error),
    #[error("Failed to connect - {0}")]
    FailedConnect(std::io::Error),
}

#[derive(Error, Debug)]
pub enum PMSServerError {
    #[error("Socket already in-use, stop existing process first")]
    SocketAlreadyInUse,
    #[error("Failed to start child process - {0}")]
    FailedToSpawnChild(pty_process::Error),
    #[error("Failed to bind to socket - {0}")]
    FailedToBind(std::io::Error),
    #[error("Failed to cleanup socket - {0}")]
    FailedRemoveSocketFile(std::io::Error),
    #[error("Failed to send msg length - {0}")]
    FailedWriteMsgLength(std::io::Error),
    #[error("Failed to send msg - {0}")]
    FailedWriteMsg(std::io::Error),
    #[error("Failed to recv msg length - {0}")]
    FailedReadMsgLength(std::io::Error),
    #[error("Failed to recv msg - {0}")]
    FailedReadMsg(std::io::Error),
    #[error("Failed to decode input msg - {0}")]
    InputDecodeError(DecodeError),
    #[error("Failed to send input to process- {0}")]
    InputFailedToSend(#[from] SendError<ProcInput>),
    #[error("Failed to write to stdin - {0}")]
    InputFailedToWrite(std::io::Error),
    #[error("Failed to send output to host - {0}")]
    OutputFailedToSend(SendError<ProcOutput>),
    #[error("Failed to read from stdout/stderr")]
    OutputFailedToRead,
    #[error("Failed to create PTY - {0}")]
    FailedCreatePTY(pty_process::Error),
}
