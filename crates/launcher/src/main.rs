//! qpad launcher
//!
//! A full-screen egui window that:
//! - Auto-detects the machine's LAN IP and encodes a session URL as a QR code.
//! - Shows which controller clients are currently connected.
//! - Optionally launches a game executable when all players are ready.
//!
//! # Usage
//!
//! ```text
//! qpad-launcher [OPTIONS] [GAME]
//!
//! Arguments:
//!   [GAME]  Path to game executable (enables the Launch Game button)
//!
//! Options:
//!   -p, --port <PORT>  Port the qpad web server is listening on [default: 3000]
//!      --host <IP>    Override the advertised server IP in the QR code
//!   -h, --help         Print help
//!   -V, --version      Print version
//! ```

use std::{net::IpAddr, path::PathBuf, sync::mpsc, time::Duration};

use clap::Parser;
use eframe::egui;
use qrcode::{QrCode, render::unicode};
use tracing::warn;
use uuid::Uuid;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "qpad-launcher",
    version,
    about = "qpad launcher — displays a QR code and manages the game session"
)]
struct Args {
    /// Port the qpad web server is listening on.
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

    /// Override the advertised server IP shown in the QR code.
    /// Defaults to the auto-detected LAN IP of this machine.
    #[arg(long, value_name = "IP")]
    host: Option<String>,

    /// Optional path to the game executable. If provided, a *Launch Game* button
    #[arg(value_name = "GAME", last = true)]
    game: Vec<String>,
}

// ── LAN IP detection ──────────────────────────────────────────────────────────

/// Detect the machine's primary LAN IP address.
///
/// Opens a UDP socket and "connects" it to an external address.  No packet is
/// ever sent — the OS just selects the outbound interface, which lets us read
/// back the local address for that route.  Works offline and on any platform.
fn detect_lan_ip() -> Option<IpAddr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // Port 53 (DNS) — any routable destination works.
    sock.connect("8.8.8.8:53").ok()?;
    Some(sock.local_addr().ok()?.ip())
}

// ── App state ─────────────────────────────────────────────────────────────────

struct LauncherApp {
    /// Full URL encoded in the QR code (uses the LAN IP so phones can reach it).
    qr_url: String,
    /// Pre-rendered Unicode QR code; `None` if generation failed.
    qr: Option<String>,
    /// Base URL for internal API calls — always loopback so it works
    /// regardless of which interface the server is bound to.
    api_base: String,
    /// Latest roster snapshot from the background poll thread.
    connected: Vec<proto::ClientInfo>,
    /// Receive-end of the background roster-polling channel.
    roster_rx: mpsc::Receiver<Vec<proto::ClientInfo>>,
    /// Optional path to the game executable. If empty, the *Launch Game* button is hidden.
    game: Vec<String>,
}

impl LauncherApp {
    fn new(
        cc: &eframe::CreationContext,
        api_base: String,
        qr_url: String,
        game: Vec<String>,
    ) -> Self {
        cc.egui_ctx.set_zoom_factor(1.5);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let qr = QrCode::new(qr_url.as_bytes())
            .ok()
            .map(|c| c.render::<unicode::Dense1x2>().build());

        // Background thread: polls /api/roster every 2 s and wakes the UI.
        let (roster_tx, roster_rx) = mpsc::channel::<Vec<proto::ClientInfo>>();
        let repaint = cc.egui_ctx.clone();
        let poll_base = api_base.clone();

        std::thread::spawn(move || {
            loop {
                if let Some(clients) = poll_roster(&poll_base) {
                    if roster_tx.send(clients).is_err() {
                        break; // receiver dropped — main window closed
                    }
                    repaint.request_repaint();
                } else {
                    warn!("roster poll failed — is qpad-web running at {poll_base}?");
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });

        Self {
            qr_url,
            qr,
            api_base,
            connected: Vec::new(),
            roster_rx,
            game,
        }
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for LauncherApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drain the roster channel — keep the latest snapshot.
        while let Ok(clients) = self.roster_rx.try_recv() {
            self.connected = clients;
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.add_space(24.0);

            // ── Header ───────────────────────────────────────────────────────

            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("qpad").size(52.0).strong());
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Scan the QR code with your phone to connect")
                        .size(15.0)
                        .color(egui::Color32::GRAY),
                );
            });

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(24.0);

            // ── QR code (left) + player list (right) ─────────────────────────

            ui.columns(2, |cols| {
                cols[0].vertical_centered(|ui| {
                    match &self.qr {
                        Some(qr) => {
                            ui.code(qr.as_str());
                        }
                        None => {
                            ui.colored_label(egui::Color32::RED, "QR generation failed");
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(&self.qr_url)
                            .monospace()
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                });

                cols[1].vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Connected players  ({})",
                            self.connected.len()
                        ))
                        .size(20.0)
                        .strong(),
                    );
                    ui.add_space(12.0);

                    if self.connected.is_empty() {
                        ui.label(
                            egui::RichText::new("Waiting for players…")
                                .size(15.0)
                                .color(egui::Color32::GRAY),
                        );
                    } else {
                        for client in &self.connected {
                            ui.group(|ui| {
                                ui.set_min_width(ui.available_width());
                                ui.label(egui::RichText::new(&client.name).size(18.0).strong());
                            });
                            ui.add_space(4.0);
                        }
                    }
                });
            });

            // ── Launch Game button (only if a game exe was provided) ──────────

            if !self.game.is_empty() {
                ui.add_space(24.0);
                ui.separator();
                ui.add_space(16.0);

                ui.vertical_centered(|ui| {
                    let btn = egui::Button::new(egui::RichText::new("▶  Launch Game").size(22.0));
                    if ui.add_sized([280.0, 54.0], btn).clicked() {
                        launch_game(self.api_base.clone(), &self.game);
                    }
                });
            }
        });
    }
}

// ── Actions ───────────────────────────────────────────────────────────────────

/// Spawn the game process and broadcast `StartGame` to all connected controllers.
///
/// Both steps are best-effort — a failure in one does not prevent the other.
fn launch_game(api_base: String, game: &[String]) {
    let path = &game[0];
    if let Err(e) = std::process::Command::new(path).args(&game[1..]).spawn() {
        tracing::error!("failed to launch {path:?}: {e}");
    }

    // POST happens in a background thread so the UI never blocks.
    std::thread::spawn(move || {
        let url = format!("{api_base}/api/game/start");
        if let Err(e) = ureq::post(&url).send_empty() {
            warn!("POST {url} failed: {e}");
        }
    });
}

// ── Polling ───────────────────────────────────────────────────────────────────

fn poll_roster(api_base: &str) -> Option<Vec<proto::ClientInfo>> {
    ureq::get(&format!("{api_base}/api/roster"))
        .call()
        .ok()?
        .into_body()
        .read_json::<proto::Roster>()
        .ok()
        .map(|r| r.clients)
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let Args { port, host, game } = Args::parse();

    // Resolve the host IP to advertise in the QR code.
    // Order of preference: --host override → auto-detected LAN IP → loopback.
    let lan_host = host.unwrap_or_else(|| {
        detect_lan_ip().map(|ip| ip.to_string()).unwrap_or_else(|| {
            warn!(
                "LAN IP detection failed — QR code will use 127.0.0.1 (phones cannot reach this)"
            );
            "127.0.0.1".to_string()
        })
    });

    // Internal API calls always use loopback (launcher and server co-locate).
    let api_base = format!("http://127.0.0.1:{}", port);
    // QR code URL uses the LAN IP so phones on the same network can connect.
    let qr_url = format!("http://{}:{}/?s={}", lan_host, port, Uuid::new_v4());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_fullscreen(true),
        ..Default::default()
    };

    eframe::run_native(
        "qpad",
        options,
        Box::new(move |cc| Ok(Box::new(LauncherApp::new(cc, api_base, qr_url, game)))),
    )
}
