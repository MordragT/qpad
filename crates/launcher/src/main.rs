//! qpad launcher
//!
//! A full-screen egui window that:
//! - Displays a QR code pointing at the local qpad web server.
//! - Shows which controller clients are currently connected.
//! - Optionally launches a game executable when all players are ready.
//!
//! # Usage
//!
//! ```
//! qpad-launcher [/path/to/game]
//! ```
//!
//! If a game path is provided, a *Launch Game* button is shown.  Clicking it
//! spawns the game process and sends `POST /api/game/start` to notify all
//! connected controllers.

use std::{path::PathBuf, sync::mpsc, time::Duration};

use eframe::egui;
use qrcode::{QrCode, render::unicode};
use tracing::warn;
use uuid::Uuid;

/// Hard-coded address of the local qpad web server.
const SERVER_ADDR: &str = "127.0.0.1:3000";

// ── App state ─────────────────────────────────────────────────────────────────

struct LauncherApp {
    /// Full session URL encoded in the QR code.
    url: String,
    /// Pre-rendered Unicode QR code; `None` if generation failed.
    qr: Option<String>,
    /// Latest roster snapshot from the background poll thread.
    connected: Vec<proto::ClientInfo>,
    /// Receive-end of the background roster-polling channel.
    roster_rx: mpsc::Receiver<Vec<proto::ClientInfo>>,
    /// Optional path to the game executable passed as a CLI argument.
    game_exe: Option<PathBuf>,
}

impl LauncherApp {
    fn new(cc: &eframe::CreationContext, game_exe: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_zoom_factor(1.5);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        // Generate a unique session URL once — it never changes.
        let url = format!("http://{}/?s={}", SERVER_ADDR, Uuid::new_v4());
        let qr = QrCode::new(url.as_bytes())
            .ok()
            .map(|c| c.render::<unicode::Dense1x2>().build());

        // Background thread: polls /api/roster every 2 s and wakes the UI.
        let (roster_tx, roster_rx) = mpsc::channel::<Vec<proto::ClientInfo>>();
        let repaint = cc.egui_ctx.clone();

        std::thread::spawn(move || {
            loop {
                if let Some(clients) = poll_roster() {
                    if roster_tx.send(clients).is_err() {
                        break; // receiver dropped — main window closed
                    }
                    repaint.request_repaint();
                } else {
                    warn!("roster poll failed — is the web server running at {SERVER_ADDR}?");
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });

        Self {
            url,
            qr,
            connected: Vec::new(),
            roster_rx,
            game_exe,
        }
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain the roster channel — keep the latest snapshot.
        while let Ok(clients) = self.roster_rx.try_recv() {
            self.connected = clients;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
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
                        egui::RichText::new(&self.url)
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

            if let Some(ref path) = self.game_exe.clone() {
                ui.add_space(24.0);
                ui.separator();
                ui.add_space(16.0);

                let enabled = !self.connected.is_empty();
                ui.vertical_centered(|ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        let btn =
                            egui::Button::new(egui::RichText::new("▶  Launch Game").size(22.0));
                        if ui.add_sized([280.0, 54.0], btn).clicked() {
                            launch_game(path.clone());
                        }
                    });

                    if !enabled {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Waiting for at least one player…")
                                .size(13.0)
                                .color(egui::Color32::GRAY),
                        );
                    }
                });
            }
        });

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

// ── Actions ───────────────────────────────────────────────────────────────────

/// Spawn the game process and notify all connected controllers.
///
/// Both operations are best-effort — errors are logged but do not abort the
/// other step.
fn launch_game(path: PathBuf) {
    // Spawn the game executable.
    if let Err(e) = std::process::Command::new(&path).spawn() {
        tracing::error!("failed to launch {path:?}: {e}");
    }

    // Notify controllers in a background thread so the UI never blocks.
    std::thread::spawn(|| {
        let url = format!("http://{SERVER_ADDR}/api/game/start");
        if let Err(e) = ureq::post(&url).call() {
            warn!("POST {url} failed: {e}");
        }
    });
}

// ── Roster polling ────────────────────────────────────────────────────────────

fn poll_roster() -> Option<Vec<proto::ClientInfo>> {
    ureq::get(&format!("http://{SERVER_ADDR}/api/roster"))
        .call()
        .ok()?
        .into_json::<proto::Roster>()
        .ok()
        .map(|r| r.clients)
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    // Optional positional argument: path to the game executable.
    let game_exe = std::env::args().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_fullscreen(true),
        ..Default::default()
    };

    eframe::run_native(
        "qpad",
        options,
        Box::new(move |cc| Ok(Box::new(LauncherApp::new(cc, game_exe)))),
    )
}
