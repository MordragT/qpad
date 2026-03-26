# qpad

Turns phones into wireless gamepads. Point your phone camera at a QR code and it opens a gamepad in the browser. Button presses are sent over WebSocket to the host machine and injected as real input events via Linux's uinput — games see a standard gamepad, no driver install needed.

## Disclaimer

This project was developed with assistance from AI-based tools (e.g., code suggestion, documentation generation).
I’m not an advocate or promoter of AI-driven development, I view it purely as a practical tool for certain tasks.

## How it works

```
phone browser  →  WebSocket  →  qpad-web  →  /dev/uinput  →  game
```

The server runs on the PC. The launcher displays a QR code with the server's LAN address. When a phone connects and registers, the server bridges its button events to a virtual input device through evdevil.

## Crates

- **proto** — shared message types used by both the server and the browser client. Builds with JSON by default (browser-compatible); disable the `json` feature for compact postcard binary encoding in production.
- **web** — Axum server. Serves the browser controller page, manages WebSocket sessions, and translates input frames to uinput key events.
- **launcher** — fullscreen egui app. Shows the QR code, lists connected players, and optionally launches a game when everyone is ready.

## Building

```bash
cargo build --release
```

Or with Nix:

```bash
nix build
```

This produces two binaries: `qpad-web` and `qpad-launcher`.

## Running

Start the server first:

```bash
qpad-web --port 3000
```

Then start the launcher from a graphical session:

```bash
qpad-launcher [--port 3000] [--host <LAN_IP>] [/path/to/game]
```

The launcher auto-detects the machine's LAN IP for the QR code. If it picks the wrong interface, pass `--host` to override. The game argument is optional — when provided, a launch button appears that becomes active once at least one player is connected.

## Permissions

`qpad-web` needs write access to `/dev/uinput`. Load the kernel module and create a udev rule:

```bash
modprobe uinput
```

```
# /etc/udev/rules.d/99-uinput.rules
KERNEL=="uinput", GROUP="input", MODE="0660"
```

Then add the user running `qpad-web` to the `input` group and re-login.

## NixOS

The flake provides a NixOS module. It enables `hardware.uinput`, creates a `qpad` system user with the right group membership, and runs `qpad-web` as a systemd service.

```nix
{
  imports = [ qpad.nixosModules.default ];

  mordrag.services.qpad = {
    enable = true;
    port = 3000;
    openFirewall = true;
  };
}
```

`qpad-launcher` is added to `environment.systemPackages` with the port pre-configured, so any user on the machine can just run:

```bash
qpad-launcher /path/to/game
```

## License

MIT or Apache-2.0 at your option.
