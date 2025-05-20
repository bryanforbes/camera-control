use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum ViscaError {
    #[error("invalid power value")]
    InvalidPowerValue,

    #[error("invalid autofocus value")]
    InvalidAutofocusValue,

    #[error("invalid zoom value")]
    InvalidZoomValue,

    #[error("invalid focus value")]
    InvalidFocusValue,

    #[error("invalid address")]
    InvalidAddress,

    #[error("invalid speed")]
    InvalidSpeed,

    #[error("invalid preset")]
    InvalidPreset,

    #[error("invalid message length")]
    InvalidMessageLength,

    #[error("syntax error")]
    Syntax,

    #[error("command buffer full")]
    CommandBufferFull,

    #[error("command canceled")]
    CommandCanceled,

    #[error("no socked")]
    NoSocket,

    #[error("command not executable")]
    CommandNotExecutable,

    #[error("unknown error")]
    Unknown,

    #[error("invalid response")]
    InvalidResponse,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Deku(#[from] deku::DekuError),
}

impl serde::Serialize for ViscaError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

pub type Result<T> = std::result::Result<T, ViscaError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_serialize() {
        assert_eq!(
            serde_json::to_string(&ViscaError::InvalidAddress).unwrap(),
            "\"invalid address\""
        );
    }
}
