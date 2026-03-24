//! REST-style API handlers.
//!
//! | Method | Path              | Description                                      |
//! |--------|-------------------|--------------------------------------------------|
//! | GET    | `/api/roster`     | Snapshot of currently-connected controller clients |
//! | POST   | `/api/game/start` | Broadcast `StartGame` to all connected clients   |

use axum::{Json, extract::State, http::StatusCode};
use proto::{Roster, ServerMsg};

use crate::state::AppState;

/// `GET /api/roster`
///
/// Returns a plain JSON [`Roster`] (not wrapped in a [`ServerMsg`] envelope),
/// making it easy to consume from non-WebSocket callers such as the launcher.
pub async fn roster(State(state): State<AppState>) -> Json<Roster> {
    let clients = state
        .clients
        .iter()
        .map(|entry| entry.value().clone())
        .collect();

    Json(Roster { clients })
}

/// `POST /api/game/start`
///
/// Broadcasts [`ServerMsg::StartGame`] to every connected WebSocket session and
/// returns `204 No Content`.  Called by the launcher when the player presses
/// the *Launch Game* button.
///
/// HTTP is stateless, but triggering a state transition (game start) through a
/// POST endpoint is idiomatic REST — the server holds state, the request just
/// commands a transition.
pub async fn game_start(State(state): State<AppState>) -> StatusCode {
    match proto::encode(&ServerMsg::StartGame) {
        Ok(bytes) => {
            let _ = state.broadcaster.send(bytes);
            StatusCode::NO_CONTENT
        }
        Err(e) => {
            tracing::error!("failed to encode StartGame: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
