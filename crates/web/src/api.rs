//! REST-style API handlers.
//!
//! | Method | Path              | Description                                      |
//! |--------|-------------------|--------------------------------------------------|
//! | GET    | `/api/roster`     | Snapshot of currently-connected controller clients |

use axum::{Json, extract::State};
use proto::Roster;

use crate::state::AppState;

/// `GET /api/roster`
///
/// Returns a plain JSON [`Roster`] (not wrapped in a [`ServerMsg`] envelope),
/// making it easy to consume from non-WebSocket callers such as the launcher.
pub async fn roster(State(state): State<AppState>) -> Json<Roster> {
    let clients = state.clients.iter().map(|entry| *entry.value()).collect();

    Json(Roster { clients })
}
