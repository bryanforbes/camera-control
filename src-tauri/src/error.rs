use thiserror::Error as ThisError;

#[derive(ThisError, Debug, specta::Type)]
pub enum Error {
    #[error("no port set")]
    NoPortSet,

    #[error(transparent)]
    Io(
        #[from]
        #[serde(skip)]
        std::io::Error,
    ),

    #[error(transparent)]
    SerialPort(
        #[from]
        #[serde(skip)]
        serialport::Error,
    ),

    #[error(transparent)]
    Visca(
        #[from]
        #[serde(skip)]
        crate::visca::Error,
    ),

    #[error(transparent)]
    Tauri(
        #[from]
        #[serde(skip)]
        tauri::Error,
    ),

    #[error(transparent)]
    Store(
        #[from]
        #[serde(skip)]
        tauri_plugin_store::Error,
    ),
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
