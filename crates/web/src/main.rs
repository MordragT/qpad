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
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

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

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let state = AppState::default();

    let app = Router::new()
        .route("/", routing::get(index))
        .route("/ws", routing::get(ws::ws_handler))
        .route("/api/roster", routing::get(api::roster))
        .with_state(state);

    info!(%addr, "qpad-web starting");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server error");
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}
