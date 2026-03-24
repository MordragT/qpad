//! WebSocket session management.
//!
//! Each connection follows this lifecycle:
//!
//! 1. Axum calls [`ws_handler`] for each upgrade request.
//! 2. [`handle_socket`] splits the socket; the sink half moves into a
//!    [`broadcast_loop`] task that forwards server-wide roster updates.
//! 3. The receive loop calls [`dispatch`] for every inbound frame.
//! 4. On disconnect the client is removed and a final roster is broadcast.
//!
//! # Codec
//!
//! [`proto::encode`] / [`proto::decode`] use whichever codec was selected at
//! compile time via the `json` feature flag (JSON by default, postcard when
//! the flag is absent).  No per-connection codec state is required.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use proto::{ClientInfo, ClientMsg, Roster, ServerMsg};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::state::AppState;

// ── Upgrade ───────────────────────────────────────────────────────────────────

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

// ── Session ───────────────────────────────────────────────────────────────────

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (ws_tx, mut ws_rx) = socket.split();

    // Subscribe before processing any frames to avoid missing a roster update
    // that races with registration.
    let brx = state.broadcaster.subscribe();
    let broadcast_task = tokio::spawn(broadcast_loop(ws_tx, brx));

    let mut client_id: Option<Uuid> = None;

    while let Some(Ok(msg)) = ws_rx.next().await {
        let bytes: &[u8] = match &msg {
            Message::Text(t) => t.as_bytes(),
            Message::Binary(b) => b,
            Message::Close(_) => break,
            _ => continue, // axum handles low-level ping/pong automatically
        };
        dispatch(bytes, &state, &mut client_id).await;
    }

    if let Some(id) = client_id {
        remove_and_broadcast(&state, id).await;
    }

    broadcast_task.abort();
    info!("ws connection closed");
}

// ── Broadcast loop ────────────────────────────────────────────────────────────

/// Forwards server-wide broadcasts (e.g. roster updates, StartGame) to the
/// WebSocket sink.  Runs as a separate task so the receive loop is never
/// blocked waiting for the network.
async fn broadcast_loop(
    mut ws_tx: SplitSink<WebSocket, Message>,
    mut brx: broadcast::Receiver<Vec<u8>>,
) {
    loop {
        match brx.recv().await {
            Ok(bytes) => {
                if ws_tx.send(Message::Binary(bytes.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("ws client lagged, dropped {n} broadcast(s)");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

async fn dispatch(bytes: &[u8], state: &AppState, client_id: &mut Option<Uuid>) {
    let msg: ClientMsg = match proto::decode(bytes) {
        Ok(m) => m,
        Err(e) => {
            debug!("unrecognized frame ({} bytes): {e}", bytes.len());
            return;
        }
    };

    match msg {
        ClientMsg::Register(reg) => {
            info!(name = %reg.name, id = %reg.client_id, "client registered");
            let info = ClientInfo {
                client_id: reg.client_id,
                name: reg.name,
                connected_at: unix_millis(),
            };
            *client_id = Some(reg.client_id);
            insert_and_broadcast(state, info).await;
        }

        ClientMsg::Input(frame) => {
            debug!(id = %frame.client_id, seq = frame.seq, "input frame");
            // Forward to the evdev bridge.  Fire-and-forget: if /dev/uinput is
            // unavailable the bridge drains silently.
            let _ = state.input_tx.send(frame);
        }
    }
}

// ── Registry helpers ──────────────────────────────────────────────────────────

async fn insert_and_broadcast(state: &AppState, info: ClientInfo) {
    state.clients.insert(info.client_id, info);
    broadcast_roster(state).await;
}

async fn remove_and_broadcast(state: &AppState, id: Uuid) {
    state.clients.remove(&id);
    broadcast_roster(state).await;
}

pub async fn broadcast_roster(state: &AppState) {
    let roster = Roster {
        clients: state.clients.iter().map(|e| e.value().clone()).collect(),
    };
    match proto::encode(&ServerMsg::Roster(roster)) {
        Ok(bytes) => {
            let _ = state.broadcaster.send(bytes);
        }
        Err(e) => error!("failed to serialize Roster: {e}"),
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
