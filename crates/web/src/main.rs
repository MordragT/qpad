//! qpad web server — entry point.
//!
//! Wires together the router, application state, and background tasks.
//! All non-trivial logic lives in submodules.

mod api;
mod input;
mod state;
mod ws;

use axum::{
    Router,
    body::Body,
    http::{HeaderValue, header},
    response::{Html, Redirect, Response},
    routing,
};
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

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let state = AppState::default();

    let app = Router::new()
        .route("/", routing::get(redirect_root))
        .route("/classic", routing::get(classic))
        .route("/analog", routing::get(analog))
        .route("/manifest.json", routing::get(manifest))
        .route("/Fredoka-SemiBold.ttf", routing::get(fredoka))
        .route("/style.css", routing::get(style_css))
        .route("/controller.js", routing::get(controller_js))
        .route("/ws", routing::get(ws::ws_handler))
        .route("/api/roster", routing::get(api::roster))
        .with_state(state);

    info!(%addr, "qpad-web starting");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server error");
}

async fn redirect_root() -> Redirect {
    Redirect::to("/classic")
}

const CLASSIC_HTML: &str = include_str!("../static/classic.html");

async fn classic() -> Html<&'static str> {
    Html(CLASSIC_HTML)
}

const ANALOG_HTML: &str = include_str!("../static/analog.html");

async fn analog() -> Html<&'static str> {
    Html(ANALOG_HTML)
}

const MANIFEST: &[u8] = include_bytes!("../static/manifest.json");

async fn manifest() -> Response {
    let mut res = Response::new(Body::from(MANIFEST));

    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/manifest+json"),
    );

    res
}

const FREDOKA: &[u8] = include_bytes!("../../../assets/fredoka/static/Fredoka-SemiBold.ttf");

async fn fredoka() -> Response {
    let mut res = Response::new(Body::from(FREDOKA));

    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("font/ttf"));
    res
}

const STYLE_CSS: &[u8] = include_bytes!("../static/style.css");

async fn style_css() -> Response {
    let mut res = Response::new(Body::from(STYLE_CSS));
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/css"));
    res
}

const CONTROLLER_JS: &[u8] = include_bytes!("../static/controller.js");

async fn controller_js() -> Response {
    let mut res = Response::new(Body::from(CONTROLLER_JS));
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript"),
    );
    res
}
