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

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use uuid::Uuid;

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

/// First message a controller client sends to identify itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Register {
    pub client_id: Uuid,
    pub name: String,
}

/// Server-side record of one connected client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub client_id: Uuid,
    pub name: String,
    /// Unix timestamp in milliseconds when the client connected.
    pub connected_at: u64,
}

/// Snapshot of all currently-connected clients, broadcast on every change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Roster {
    pub clients: Vec<ClientInfo>,
}

/// A single input frame from a controller client (see bitmask table in module docs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFrame {
    pub client_id: Uuid,
    /// Monotonically-increasing per-client sequence number (starts at 1).
    pub seq: u64,
    /// Bitmask of currently-pressed buttons.
    pub buttons: u32,
    /// Analogue axes, conventionally in `[-32768, 32767]`.
    pub axes: Vec<i16>,
    /// Unix timestamp in milliseconds when the frame was captured.
    pub ts_millis: u64,
}

// ── Envelope types ────────────────────────────────────────────────────────────

/// Every message a controller client may send to the server.
///
/// Encoded with serde's default *externally-tagged* format:
/// `{"Register":{"client_id":"…","name":"…"}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMsg {
    Register(Register),
    Input(InputFrame),
}

/// Every message the server may send to a controller client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Updated snapshot of all connected clients.
    Roster(Roster),
    /// Sent by the server when the game session begins.
    ///
    /// Triggered by `POST /api/game/start` from the launcher.
    StartGame,
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
