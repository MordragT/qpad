//! Shared application state threaded through every Axum handler.

use dashmap::DashMap;
use proto::ClientInfo;
use std::sync::Arc;
use uuid::Uuid;

/// Convenience alias: the concurrent map that tracks connected clients.
pub type Clients = Arc<DashMap<Uuid, ClientInfo>>;

/// Cheap-to-clone state shared across every connection handler.
///
/// All fields are either `Arc`-wrapped or trivially `Clone`, so cloning
/// `AppState` for each handler is zero-copy at the data level.
#[derive(Clone, Default)]
pub struct AppState {
    /// Live registry of every currently-connected controller client.
    pub clients: Clients,
}
