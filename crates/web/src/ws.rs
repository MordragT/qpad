//! WebSocket session management.
//!
//! Each connection follows this life cycle:
//!
//! 1. Axum calls [`ws_handler`] for each upgrade request.
//! 2. The receive loop calls [`dispatch`] for every inbound frame.
//! 3. On disconnect the client is removed.
//!
//! # Codec
//!
//! [`proto::encode`] / [`proto::decode`] use whichever codec was selected at
//! compile time via the `json` feature flag (JSON by default, postcard when
//! the flag is absent). No per-connection codec state is required.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use proto::{ClientInfo, ClientMsg};
use tracing::{debug, info};

use crate::{input::Controller, state::AppState};

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

#[derive(Debug)]
pub struct SessionState {
    info: ClientInfo,
    controller: Controller,
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut session: Option<SessionState> = None;

    while let Some(Ok(msg)) = socket.recv().await {
        let bytes: &[u8] = match &msg {
            Message::Text(t) => t.as_bytes(),
            Message::Binary(b) => b,
            Message::Close(_) => break,
            _ => continue, // axum handles low-level ping/pong automatically
        };
        dispatch(bytes, &state, &mut session).await;
    }

    if let Some(session) = session {
        state.clients.remove(&session.info.client_id);
    }

    info!("ws connection closed");
}

async fn dispatch(bytes: &[u8], state: &AppState, session: &mut Option<SessionState>) {
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

            let info = ClientInfo::from(reg);
            let controller = match Controller::open(&info.name) {
                Ok(c) => c,
                Err(e) => {
                    debug!(
                        "failed to open controller for client {}: {e}",
                        info.client_id
                    );
                    return;
                }
            };

            state.clients.insert(info.client_id, info.clone());
            *session = Some(SessionState { info, controller });
        }
        ClientMsg::Input(frame) => {
            debug!(id = %frame.client_id, seq = frame.seq, "input frame");

            let Some(session) = session else {
                debug!("received input frame before registration, ignoring");
                return;
            };

            session.controller.handle_frame(frame);
        }
    }
}
