//! Virtual gamepad input bridge.
//!
//! Spawns a background task that owns a [`UinputDevice`] and translates
//! incoming [`InputFrame`] messages into kernel input events via evdevil.
//!
//! # Button layout
//!
//! Bit positions in [`InputFrame::buttons`] map to evdev button codes as
//! documented in [`proto::InputFrame`].  Only pressed/released *transitions*
//! are forwarded to the kernel — holding a button does not spam repeat events.
//!
//! # Permissions
//!
//! Creating a uinput device requires write access to `/dev/uinput`.
//! Either run the server as root, or add a udev rule such as:
//!
//! ```text
//! KERNEL=="uinput", GROUP="input", MODE="0660"
//! ```
//!
//! and add your user to the `input` group.  When permissions are missing the
//! task logs a warning and drains the channel silently so the rest of the
//! server continues to function.

use std::collections::HashMap;

use evdevil::{
    event::{InputEvent, Key, KeyEvent, KeyState},
    uinput::UinputDevice,
};
use proto::InputFrame;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

// ── Button map ────────────────────────────────────────────────────────────────

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

// ── Public interface ──────────────────────────────────────────────────────────

/// Spawn the input bridge task and return the sending end of its channel.
///
/// Drop the sender (or close the channel) to shut down the task gracefully.
/// Spawn the input bridge task and return the sending end of its channel.
///
/// Drop the sender (or close the channel) to shut down the task gracefully.
pub fn start() -> mpsc::UnboundedSender<InputFrame> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run(rx));
    tx
}

// ── Task implementation ───────────────────────────────────────────────────────

async fn run(mut rx: mpsc::UnboundedReceiver<InputFrame>) {
    match open_virtual_gamepad() {
        Ok(device) => {
            info!(
                sysname = ?device.sysname().ok(),
                "virtual gamepad created",
            );
            run_with_device(device, rx).await;
        }
        Err(e) => {
            warn!(
                "could not open /dev/uinput ({e}); \
                 add udev rule: KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\" \
                 or run as root — input forwarding disabled"
            );
            // Drain the channel so senders never block on a full buffer.
            while rx.recv().await.is_some() {}
        }
    }
}

async fn run_with_device(device: UinputDevice, mut rx: mpsc::UnboundedReceiver<InputFrame>) {
    // Per-client button state, used to compute press/release deltas.
    let mut prev: HashMap<Uuid, u32> = HashMap::new();

    while let Some(frame) = rx.recv().await {
        let old = *prev.get(&frame.client_id).unwrap_or(&0);
        let new = frame.buttons;

        if old == new {
            continue; // nothing changed, skip the write
        }

        let events = key_events(old, new);

        if !events.is_empty() {
            // UinputDevice::write is a synchronous syscall to /dev/uinput.
            // It should return almost immediately (kernel buffer), but for a
            // production implementation consider spawn_blocking.
            if let Err(e) = device.write(&events) {
                error!("evdev write failed: {e}");
            }
        }

        prev.insert(frame.client_id, new);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Open and configure the virtual gamepad device.
fn open_virtual_gamepad() -> std::io::Result<UinputDevice> {
    let keys = BUTTON_MAP.iter().map(|&(_, key)| key);

    UinputDevice::builder()?
        .with_keys(keys)?
        .build("qpad virtual gamepad")
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
