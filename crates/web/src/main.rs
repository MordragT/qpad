//! qpad web server — entry point.
//!
//! Wires together the router, application state, and background tasks.
//! All non-trivial logic lives in submodules.

mod api;
mod input;
mod state;
mod ws;

use axum::{Router, response::Html, routing};
use clap::Parser;
use state::AppState;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tracing::info;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "qpad-web",
    version,
    about = "qpad web server — serves the controller client and bridges gamepad input to uinput"
)]
struct Args {
    /// Port to listen on.
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

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

    let args = Args::parse();
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));

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
        .route("/", routing::get(index))
        .route("/ws", routing::get(ws::ws_handler))
        .route("/api/roster", routing::get(api::roster))
        .route("/api/game/start", routing::post(api::game_start))
        .with_state(state);

    info!(%addr, "qpad-web starting");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server error");
}

// ── Index handler ─────────────────────────────────────────────────────────────

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}
