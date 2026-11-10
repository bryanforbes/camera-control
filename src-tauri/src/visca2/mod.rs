//! RS-232 VISCA camera sessions.

mod adapter;
mod codec;
mod transport;

#[allow(unused_imports)] // The UI does not select VISCA until hardware validation.
pub(crate) use adapter::open;
