use serde::Serialize;
use specta::Type;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug, Serialize, Type)]
#[serde(tag = "type", content = "data", rename = "CameraError")]
pub enum Error {
    #[error("Camera command queue is full")]
    CameraBusy,

    #[error("Camera closed")]
    CameraSessionClosed,

    #[error("Worker start failed")]
    CameraWorkerStart,

    #[error("Failed to receive camera response")]
    CameraResponseReceiveFailed,

    #[error("Motion IDs exhausted")]
    CameraMotionIdsExhausted,

    #[error("Camera protocol error: {0}")]
    Protocol(
        #[serde(skip)]
        #[from]
        Box<dyn std::error::Error + Send + Sync>,
    ),

    #[error("SerialPort error: {0}")]
    SerialPort(
        #[serde(skip)]
        #[from]
        serialport::Error,
    ),

    #[error("SerialPort error: {0}")]
    Io(
        #[serde(skip)]
        #[from]
        std::io::Error,
    ),
}

pub type Result<T> = std::result::Result<T, Error>;
