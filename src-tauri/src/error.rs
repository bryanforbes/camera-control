use serde::Serialize;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug, Serialize, specta::Type)]
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

    #[error(transparent)]
    PelcoD(
        #[from]
        #[serde(skip)]
        pelcodrs::Error,
    ),
}

pub type Result<T> = std::result::Result<T, Error>;
