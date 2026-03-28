//! Virtual gamepad input bridge.
//!
//! This module provides a Controller that owns a [`UinputDevice`] and
//! translates [`InputFrame`] messages into kernel input events via evdevil.
//! The caller is expected to open a `Controller` and call [`Controller::handle_frame`]
//! whenever a new frame arrives.
//!
//! # Button layout
//!
//! Bit positions in [`InputFrame::buttons`] map to evdev button codes as
//! documented in [`proto::InputFrame`]. Only pressed/released *transitions*
//! are forwarded to the kernel — holding a button does not spam repeat events.
//!
//! # Permissions
//!
//! Creating a uinput device requires write access to `/dev/uinput`. The
//! constructor [`Controller::open`] returns a `std::io::Result` and will
//! fail if permissions are insufficient. Either run the server as root, or
//! add a udev rule such as:
//!
//! ```text
//! KERNEL=="uinput", GROUP="input", MODE="0660"
//! ```
//!
//! and add your user to the `input` group. Writes performed by
//! [`Controller::handle_frame`] are synchronous syscalls to `/dev/uinput`
//! and may return errors which the controller logs.

use evdevil::{
    event::{InputEvent, Key, KeyEvent, KeyState},
    uinput::UinputDevice,
};
use proto::InputFrame;
use tracing::error;

/// Maps each button-bitmask bit to the corresponding evdev [`Key`] code.
///
/// The order must match the bitmask layout documented in [`proto::InputFrame`].
const BUTTON_MAP: [(u32, Key); 10] = [
    (1 << 0, Key::BTN_SOUTH), // A
    (1 << 1, Key::BTN_EAST),  // B
    (1 << 2, Key::BTN_NORTH), // Y
    (1 << 3, Key::BTN_WEST),  // X
    (1 << 4, Key::BTN_START),
    (1 << 5, Key::BTN_SELECT),
    (1 << 6, Key::BTN_DPAD_UP),
    (1 << 7, Key::BTN_DPAD_DOWN),
    (1 << 8, Key::BTN_DPAD_LEFT),
    (1 << 9, Key::BTN_DPAD_RIGHT),
];

#[derive(Debug)]
pub struct Controller {
    device: UinputDevice,
    state: u32,
}

impl Controller {
    /// Open and configure the virtual gamepad device.
    pub fn open(name: &str) -> std::io::Result<Self> {
        let keys = BUTTON_MAP.iter().map(|&(_, key)| key);
        let device = UinputDevice::builder()?.with_keys(keys)?.build(name)?;
        device.set_nonblocking(true)?;

        Ok(Self { device, state: 0 })
    }

    /// Handle a new input frame by computing the button state transitions and
    /// writing the corresponding events to the kernel.
    pub fn handle_frame(&mut self, frame: InputFrame) {
        let new = frame.buttons;

        if self.state == new {
            return; // nothing changed, skip the write
        }

        let events = key_events(self.state, new);

        if !events.is_empty() {
            // UinputDevice::write is a synchronous syscall to /dev/uinput.
            // It should return almost immediately (kernel buffer)
            if let Err(e) = self.device.write(&events) {
                error!("evdev write failed: {e}");
            }
        }

        self.state = new;
    }
}

/// Compute the set of [`InputEvent`]s needed to transition from button state
/// `old` to `new`, emitting a `PRESSED` event for each newly-set bit and a
/// `RELEASED` event for each newly-cleared bit.
fn key_events(old: u32, new: u32) -> Vec<InputEvent> {
    let mut events = Vec::new();

    for &(bit, key) in &BUTTON_MAP {
        let was_pressed = old & bit != 0;
        let is_pressed = new & bit != 0;

        match (was_pressed, is_pressed) {
            (false, true) => events.push(KeyEvent::new(key, KeyState::PRESSED).into()),
            (true, false) => events.push(KeyEvent::new(key, KeyState::RELEASED).into()),
            _ => {}
        }
    }
    events
}
