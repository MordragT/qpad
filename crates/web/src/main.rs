//! qpad web server — entry point.
//!
//! Wires together the router, application state, and background tasks.
//! All non-trivial logic lives in submodules.
//!
//! # Environment variables
//!
//! | Variable     | Default        | Description                              |
//! |--------------|----------------|------------------------------------------|
//! | `QPAD_BIND`  | `0.0.0.0:3000` | TCP address to listen on                 |

mod api;
mod input;
mod state;
mod ws;

use axum::{Router, response::Html, routing::get};
use state::AppState;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tracing::info;

// ── Embedded static asset ─────────────────────────────────────────────────────
//
// The entire controller client is a single self-contained HTML file with
// inline CSS and JS.  Embedding it at compile time means no runtime path
// dependency and instant startup.  Cargo rebuilds automatically when the
// file changes.

const INDEX_HTML: &str = include_str!("../static/index.html");

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = std::env::var("QPAD_BIND")
        .unwrap_or_else(|_| "0.0.0.0:3000".into())
        .parse()
        .expect("QPAD_BIND must be a valid <ip>:<port>");

    // Small broadcast buffer — roster updates are infrequent.
    let (broadcaster, _) = broadcast::channel::<Vec<u8>>(16);

    // Spawn the evdev input bridge; the sender end lives in AppState.
    let input_tx = input::start();

    let state = AppState {
        clients: Arc::new(dashmap::DashMap::new()),
        broadcaster,
        input_tx,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws::ws_handler))
        .route("/api/roster", get(api::roster))
        .route("/api/game/start", axum::routing::post(api::game_start))
        .with_state(state);

    info!(%addr, "qpad server starting");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server error");
}

// ── Index handler ─────────────────────────────────────────────────────────────

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}
