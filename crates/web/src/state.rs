//! Shared application state threaded through every Axum handler.

use dashmap::DashMap;
use proto::{ClientInfo, InputFrame};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

/// Convenience alias: the concurrent map that tracks connected clients.
pub type Clients = Arc<DashMap<Uuid, ClientInfo>>;

/// Cheap-to-clone state shared across every connection handler.
///
/// All fields are either `Arc`-wrapped or trivially `Clone`, so cloning
/// `AppState` for each handler is zero-copy at the data level.
#[derive(Clone)]
pub struct AppState {
    /// Live registry of every currently-connected controller client.
    pub clients: Clients,

    /// Broadcast channel carrying serialized [`proto::ServerMsg`] bytes.
    ///
    /// Every WebSocket session subscribes to this on connect; the server
    /// broadcasts an updated [`proto::Roster`] whenever a client joins or
    /// leaves.  The payload is always JSON-encoded so that browser clients
    /// can decode it without any special handling.
    pub broadcaster: broadcast::Sender<Vec<u8>>,

    /// One-shot channel into the evdev input bridge task.
    ///
    /// The WebSocket dispatch loop forwards every [`InputFrame`] it receives
    /// into this sender; the bridge task translates them into kernel input
    /// events via [`evdevil`].
    pub input_tx: mpsc::UnboundedSender<InputFrame>,
}
