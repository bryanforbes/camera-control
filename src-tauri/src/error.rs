use serde::Serialize;
use specta::Type;
use thiserror::Error as ThisError;

use crate::camera::Error as CameraError;

#[derive(ThisError, Debug, Serialize, Type)]
#[serde(tag = "type", content = "data")]
pub enum Error {
    #[error("No port set")]
    NoPortSet,

    #[error("{0}")]
    Camera(#[from] CameraError),

    #[error("Tauri error: {0}")]
    Tauri(
        #[serde(skip)]
        #[from]
        tauri::Error,
    ),

    #[error("Store error: {0}")]
    Store(
        #[serde(skip)]
        #[from]
        tauri_plugin_store::Error,
    ),

    #[error("Store error: {0}")]
    Io(
        #[serde(skip)]
        #[from]
        std::io::Error,
    ),

    #[error("SerialPort error: {0}")]
    SerialPort(
        #[serde(skip)]
        #[from]
        serialport::Error,
    ),
}

pub type Result<T> = std::result::Result<T, Error>;
