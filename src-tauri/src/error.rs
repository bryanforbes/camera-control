use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("no port set")]
    NoPortSet,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    SerialPort(#[from] serialport::Error),

    #[error(transparent)]
    Visca(#[from] crate::visca::Error),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),

    #[error(transparent)]
    Store(#[from] tauri_plugin_store::Error),
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
