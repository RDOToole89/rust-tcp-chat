// src/errors.rs
use std::io;
use std::sync::PoisonError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatServerError {
    #[error("IO error occurred: {0}")]
    IoError(#[from] io::Error),
    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Client disconnected: {0}")]
    ClientDisconnected(String),
    #[error("A shared resource lock was poisoned")]
    PoisonedLock,
}

/// A type alias for results returned by the chat server functions.
pub type ChatResult<T> = Result<T, ChatServerError>;

impl<T> From<PoisonError<T>> for ChatServerError {
    fn from(_: PoisonError<T>) -> Self {
        ChatServerError::PoisonedLock
    }
}
