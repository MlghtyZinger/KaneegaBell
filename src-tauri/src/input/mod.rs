//! Controller input: a background SDL3 thread that turns gamepad events into
//! UI navigation actions and watches for the exit-emulator combo.

pub mod gamepad;
pub mod logic;

pub use gamepad::{ExitCombo, InputShared};
