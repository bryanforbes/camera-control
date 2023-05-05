use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("invalid address")]
    InvalidAddress,

    #[error("invalid speed")]
    InvalidSpeed,

    #[error("invalid preset")]
    InvalidPreset,

    #[error("invalid message length")]
    InvalidMessageLength,

    #[error("syntax error")]
    SyntaxError,

    #[error("command buffer full")]
    CommandBufferFull,

    #[error("command canceled")]
    CommandCanceled,

    #[error("no socked")]
    NoSocket,

    #[error("command not executable")]
    CommandNotExecutable,

    #[error("unknown message error")]
    UnknownMessageError,

    #[error("invalid response")]
    InvalidResponse,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
