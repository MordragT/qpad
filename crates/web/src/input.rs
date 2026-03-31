//! Virtual gamepad input bridge.
//!
//! This module provides a Qpad that owns a [`UinputDevice`] and
//! translates [`InputFrame`] messages into kernel input events via evdevil.
//! The caller is expected to open a `Qpad` and call [`Qpad::handle_frame`]
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
    AbsInfo,
    event::{Abs, AbsEvent, Key, KeyEvent, KeyState},
    uinput::{AbsSetup, UinputDevice},
};
use proto::{ButtonSet, ClientInfo, InputFrame};
use smallvec::SmallVec;
use tracing::error;

#[derive(Debug)]
pub struct Qpad {
    device: UinputDevice,
    buttons: ButtonSet,
}

impl Qpad {
    /// Open and configure the virtual gamepad device.
    pub fn open(info: ClientInfo) -> std::io::Result<Self> {
        let ClientInfo {
            id,
            layout,
            connected_at: _,
        } = info;

        let mut device = UinputDevice::builder()?
            .with_input_id(layout.input_id())?
            .with_keys(layout.buttons().into_iter().map(Key::from))?;

        if layout.axes() {
            device = device.with_abs_axes([
                AbsSetup::new(Abs::X, AbsInfo::new(-32767, 32767).with_flat(128)),
                AbsSetup::new(Abs::Y, AbsInfo::new(-32767, 32767).with_flat(128)),
            ])?;
        }

        let device = device.build(&format!("Qpad {layout} ({id})"))?;
        device.set_nonblocking(true)?;

        Ok(Self {
            device,
            buttons: ButtonSet::empty(),
        })
    }

    /// Handle a new input frame by computing the button state transitions and
    /// writing the corresponding events to the kernel.
    pub fn handle_frame(&mut self, frame: InputFrame) {
        let InputFrame {
            buttons: new_buttons,
            x_axis,
            y_axis,
            ..
        } = frame;

        if self.buttons == new_buttons
            && x_axis <= 128
            && x_axis >= -128
            && y_axis <= 128
            && y_axis >= -128
        {
            // no change since last frame, skip writing events
            return;
        }

        // TODO: make this less magic with type safety
        // classic max events = 8
        // analog max events = 6 buttons + 2 axes = 8
        let mut events = SmallVec::<[_; 8]>::new();

        // pressed in new
        for button in new_buttons.difference(self.buttons) {
            events.push(KeyEvent::new(button.into(), KeyState::PRESSED).into());
        }

        // released in new
        for button in self.buttons.difference(new_buttons) {
            events.push(KeyEvent::new(button.into(), KeyState::RELEASED).into());
        }

        events.push(AbsEvent::new(Abs::X, x_axis as i32).into());
        events.push(AbsEvent::new(Abs::Y, y_axis as i32).into());

        if let Err(e) = self.device.write(&events) {
            error!("evdev write failed: {e}");
        }

        self.buttons = new_buttons;
    }
}
