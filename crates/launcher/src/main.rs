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

use std::{net::IpAddr, os::unix::process::CommandExt, sync::mpsc, time::Duration};

use clap::Parser;
use eframe::egui;
use egui::{
    ColorImage, FontData, FontFamily, Image, TextureHandle, TextureOptions,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
};
use proto::{ClientInfo, QpadLayout};
use qrcode::QrCode;
use tracing::warn;
use twemoji_assets::svg::SvgTwemojiAsset;

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

    /// Override the advertised server IP.
    #[arg(long, value_name = "IP")]
    host: Option<String>,

    /// Controller layout.
    #[arg(long, value_name = "LAYOUT", default_value_t  = QpadLayout::Classic)]
    layout: QpadLayout,

    /// Optional path to the game executable.
    #[arg(value_name = "GAME", last = true)]
    game: Vec<String>,

    /// Fullscreen mode
    #[arg(long, default_value_t = false)]
    fullscreen: bool,
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
    /// Pre-rendered QR code; `None` if generation failed.
    qr: Option<TextureHandle>,
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
            .map(|c| c.render::<image::Rgba<u8>>().build())
            .map(|img| {
                let size = [img.width() as usize; 2];
                let color = ColorImage::from_rgba_unmultiplied(size, &img);
                cc.egui_ctx
                    .load_texture("qr", color, TextureOptions::LINEAR)
            });

        // Background thread: polls /api/roster every 2 s and wakes the UI.
        let (roster_tx, roster_rx) = mpsc::channel::<Vec<proto::ClientInfo>>();
        let repaint = cc.egui_ctx.clone();

        std::thread::spawn(move || {
            loop {
                if let Some(clients) = poll_roster(&api_base) {
                    if roster_tx.send(clients).is_err() {
                        break; // receiver dropped — main window closed
                    }
                    repaint.request_repaint();
                } else {
                    warn!("roster poll failed — is qpad-web running at {api_base}?");
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });

        Self {
            qr_url,
            qr,
            connected: Vec::new(),
            roster_rx,
            game,
        }
    }
}

// ── eframe::App ───────────────────────────────────────────────────────────────

// TODO put in proto and make 3 level name list e.g. ["The", "A"], ["Greatest", "Latest"], ["King", "Tiger"]

const PLAYERS: &[(&str, &str)] = &[
    ("🦊", "Fox"),
    ("🐺", "Wolf"),
    ("🐻", "Bear"),
    ("🦁", "Lion"),
    ("🐯", "Tiger"),
    ("🦅", "Eagle"),
    ("🦈", "Shark"),
    ("🦉", "Owl"),
    ("🐸", "Frog"),
    ("🐉", "Drake"),
    ("🦝", "Rascal"),
    ("🐬", "Finn"),
    ("🦋", "Blaze"),
    ("🐙", "Ink"),
    ("🦌", "Buck"),
    ("🐆", "Spot"),
];

fn emoji_source(emoji: &str) -> egui::ImageSource<'static> {
    let svg = SvgTwemojiAsset::from_emoji(emoji).expect("failed to load twemoji asset");

    egui::ImageSource::Bytes {
        uri: std::borrow::Cow::Owned(format!("bytes://twemoji/{emoji}.svg")),
        bytes: egui::load::Bytes::Static(svg.as_bytes()),
    }
}

#[derive(Clone, Copy)]
pub enum BadgeSide {
    Left,
    Right,
}

fn player_badge(ui: &mut egui::Ui, client: &ClientInfo, side: BadgeSide) {
    let [r, g, b] = client.id.into_inner();
    let bg = egui::Color32::from_rgb(r, g, b);

    // WCAG-style luma to pick a readable text colour
    let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    let fg = if luma > 140.0 {
        egui::Color32::from_black_alpha(210)
    } else {
        egui::Color32::WHITE
    };

    let idx =
        ((r as usize) ^ (g as usize).rotate_left(3) ^ (b as usize).rotate_left(6)) % PLAYERS.len();
    let (icon, name) = PLAYERS[idx];

    let emoji = emoji_source(icon);

    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(egui::Color32::WHITE)
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(4))
            .outer_margin(egui::Margin::symmetric(80, 16))
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| match side {
                        BadgeSide::Left => {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;

                                ui.add(
                                    egui::Image::new(emoji)
                                        .fit_to_exact_size(egui::Vec2::splat(28.0)),
                                );
                                ui.label(egui::RichText::new(name).size(16.0).strong().color(fg));
                            })
                        }
                        BadgeSide::Right => {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;

                                ui.add(
                                    egui::Image::new(emoji)
                                        .fit_to_exact_size(egui::Vec2::splat(28.0)),
                                );
                                ui.label(egui::RichText::new(name).size(16.0).strong().color(fg));
                            })
                        }
                    });
            });
    });
}

impl eframe::App for LauncherApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(500));
        ctx.add_font(FontInsert::new(
            "fredoka",
            FontData::from_static(include_bytes!(
                "../../../assets/fredoka/static/Fredoka-SemiBold.ttf"
            )),
            vec![InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Highest,
            }],
        ));
        egui_extras::install_image_loaders(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drain the roster channel — keep the latest snapshot.
        while let Ok(clients) = self.roster_rx.try_recv() {
            self.connected = clients;
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                // Launch button pinned to bottom
                if !self.game.is_empty() {
                    ui.add_space(16.0);
                    let btn = egui::Button::new(egui::RichText::new("▶  Launch Game").size(20.0));
                    if ui.add_sized([260.0, 48.0], btn).clicked() {
                        launch_game(&self.game);
                    }
                    ui.add_space(16.0);
                }

                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(16.0);

                    // ── Header ──────────────────────────────────────────────────
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("qpad").size(64.0).strong());
                        ui.add_space(16.0);
                        ui.label(
                            egui::RichText::new("Scan the QR code with your phone to connect")
                                .size(20.0)
                                .color(egui::Color32::GRAY),
                        );
                    });

                    ui.add_space(16.0);

                    // ── Badges (left edge) | QR (center) | Badges (right edge) ──
                    let mid = (self.connected.len() + 1) / 2;
                    let (left_players, right_players) = self.connected.split_at(mid);

                    ui.columns(3, |cols| {
                        // Left badges — right-aligned so they hug the QR
                        cols[0].with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                            ui.add_space(8.0);
                            for client in left_players {
                                player_badge(ui, client, BadgeSide::Left);
                                ui.add_space(8.0);
                            }
                        });

                        // QR + URL centered
                        cols[1].vertical_centered(|ui| {
                            ui.add_space(128.0);

                            match &self.qr {
                                Some(qr) => {
                                    ui.add(
                                        egui::Image::from_texture(qr)
                                            .corner_radius(egui::CornerRadius::same(14)),
                                    );
                                }
                                None => {
                                    ui.colored_label(egui::Color32::RED, "QR generation failed");
                                }
                            }
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(&self.qr_url)
                                    .monospace()
                                    .size(10.0)
                                    .color(egui::Color32::GRAY),
                            );
                        });

                        // Right badges — left-aligned so they hug the QR
                        cols[2].with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.add_space(8.0);
                            for client in right_players {
                                player_badge(ui, client, BadgeSide::Right);
                                ui.add_space(8.0);
                            }
                        });
                    });
                });
            });
        });
    }
}

/// Spawn the game process and broadcast `StartGame` to all connected controllers.
///
/// Both steps are best-effort — a failure in one does not prevent the other.
fn launch_game(game: &[String]) {
    let path = &game[0];
    if let Some(e) = std::process::Command::new(path)
        .args(&game[1..])
        .exec()
        .raw_os_error()
    {
        tracing::error!("failed to launch {path:?}: {e}");
    }
}

fn poll_roster(api_base: &str) -> Option<Vec<proto::ClientInfo>> {
    ureq::get(&format!("{api_base}/api/roster"))
        .call()
        .ok()?
        .into_body()
        .read_json::<proto::Roster>()
        .ok()
        .map(|r| r.clients)
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let Args {
        port,
        host,
        game,
        layout,
        fullscreen,
    } = Args::parse();

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
    let qr_url = format!("http://{}:{}/{}", lan_host, port, layout);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_fullscreen(fullscreen),
        ..Default::default()
    };

    eframe::run_native(
        "qpad",
        options,
        Box::new(move |cc| Ok(Box::new(LauncherApp::new(cc, api_base, qr_url, game)))),
    )
}
