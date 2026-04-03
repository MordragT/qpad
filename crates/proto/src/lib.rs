//! Shared protocol types for qpad.
//!
//! # Wire format
//!
//! All messages are wrapped in an envelope enum:
//!
//! - Client → server: [`ClientMsg`]
//! - Server → client: [`ServerMsg`]
//!
//! The codec is selected at **compile time** via the `json` feature flag:
//!
//! | Build            | Feature       | Codec     | Use case                    |
//! |------------------|---------------|-----------|-----------------------------|
//! | debug (default)  | `json`        | JSON      | Browser clients, easy debug |
//! | release          | *(none)*      | postcard  | Native clients, production  |
//!
//! Switch to postcard by disabling default features:
//! ```toml
//! proto = { workspace = true, default-features = false }
//! ```
//!
//! # Button bitmask (InputFrame::buttons)
//!
//! | Bit | evdev key        | Label  |
//! |-----|------------------|--------|
//! | 0   | `BTN_SOUTH`      | A      |
//! | 1   | `BTN_EAST`       | B      |
//! | 2   | `BTN_NORTH`      | Y      |
//! | 3   | `BTN_WEST`       | X      |
//! | 4   | `BTN_START`      | Start  |
//! | 5   | `BTN_SELECT`     | Select |
//! | 6   | `BTN_DPAD_UP`    | D↑     |
//! | 7   | `BTN_DPAD_DOWN`  | D↓     |
//! | 8   | `BTN_DPAD_LEFT`  | D←     |
//! | 9   | `BTN_DPAD_RIGHT` | D→     |

use std::{fmt, str::FromStr};

use clap::ValueEnum;
use evdevil::{Bus, InputId};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

mod button;

pub use button::{Button, ButtonSet};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("postcard: {0}")]
    Postcard(postcard::Error),

    #[cfg(feature = "json")]
    #[error("json: {0}")]
    Json(serde_json::Error),
}

impl From<postcard::Error> for ProtoError {
    fn from(e: postcard::Error) -> Self {
        Self::Postcard(e)
    }
}

#[cfg(feature = "json")]
impl From<serde_json::Error> for ProtoError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ── Leaf types ────────────────────────────────────────────────────────────────

// RGB Client Id
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClientId([u8; 3]);

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        write!(f, "{s}")
    }
}

impl ClientId {
    pub fn into_inner(self) -> [u8; 3] {
        self.0
    }
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum QpadLayout {
    #[default]
    Classic,
    Analog,
}

impl FromStr for QpadLayout {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "classic" => Ok(Self::Classic),
            "analog" => Ok(Self::Analog),
            _ => Err(format!("invalid client kind: {s}")),
        }
    }
}

impl fmt::Display for QpadLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Classic => "classic",
            Self::Analog => "analog",
        };
        write!(f, "{s}")
    }
}

impl QpadLayout {
    pub const fn input_id(self) -> InputId {
        match self {
            Self::Classic => InputId::new(Bus::USB, 0x1209, 0x2881, 0x0100),
            Self::Analog => InputId::new(Bus::USB, 0x1209, 0x2882, 0x0100),
        }
    }

    pub fn buttons(self) -> ButtonSet {
        match self {
            Self::Classic => {
                Button::A
                    | Button::B
                    | Button::Y
                    | Button::X
                    | Button::Start
                    | Button::Select
                    | Button::Up
                    | Button::Down
                    | Button::Left
                    | Button::Right
            }
            Self::Analog => {
                Button::A | Button::B | Button::Y | Button::X | Button::Start | Button::Select
            }
        }
    }

    pub fn axes(self) -> bool {
        match self {
            Self::Classic => false,
            Self::Analog => true,
        }
    }
}

/// First message a controller client sends to identify itself.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Register {
    pub id: ClientId,
    pub layout: QpadLayout,
}

/// Server-side record of one connected client.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: ClientId,
    pub layout: QpadLayout,
    /// Unix timestamp in milliseconds when the client connected.
    pub connected_at: u64,
}

impl From<Register> for ClientInfo {
    fn from(reg: Register) -> Self {
        let Register { id, layout } = reg;

        Self {
            id,
            layout,
            connected_at: unix_millis(),
        }
    }
}

/// Snapshot of all currently-connected clients, broadcast on every change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roster {
    pub clients: Vec<ClientInfo>,
}

/// A single button input frame from a controller (see bitmask table in module docs).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InputFrame {
    pub id: ClientId,
    /// Bitmask of currently-pressed buttons.
    pub buttons: ButtonSet,
    /// X axis for analog stick
    pub x_axis: i16,
    /// Y axis for analog stick
    pub y_axis: i16,
    /// Unix timestamp in milliseconds when the frame was captured.
    pub timestamp: u64,
}

// ── Envelope types ────────────────────────────────────────────────────────────

/// Every message a controller client may send to the server.
///
/// Encoded with serde's default *externally-tagged* format:
/// `{"Register":{"client_id":"…","name":"…"}}`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ClientMsg {
    Register(Register),
    Input(InputFrame),
}

/// Every message the server may send to a controller client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Updated snapshot of all connected clients.
    Roster(Roster),
}

// Small helper to get the current Unix timestamp in milliseconds.
fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Codec ─────────────────────────────────────────────────────────────────────

/// Serialize `value` to bytes using the compile-time selected codec.
///
/// - With the `json` feature (default): UTF-8 JSON via `serde_json`.
/// - Without `json`: compact binary via `postcard`.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtoError> {
    #[cfg(feature = "json")]
    {
        Ok(serde_json::to_vec(value)?)
    }
    #[cfg(not(feature = "json"))]
    {
        Ok(postcard::to_stdvec(value)?)
    }
}

/// Deserialize `T` from raw bytes using the compile-time selected codec.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProtoError> {
    #[cfg(feature = "json")]
    {
        Ok(serde_json::from_slice(bytes)?)
    }
    #[cfg(not(feature = "json"))]
    {
        Ok(postcard::from_bytes(bytes)?)
    }
}
