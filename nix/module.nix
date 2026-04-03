# NixOS module for qpad.
#
# Provides:
#   options.mordrag.services.qpad.*  — configuration knobs
#   systemd.services.qpad-web        — headless server + uinput bridge
#   environment.systemPackages       — port-aware qpad-launcher wrapper
#
# Minimal host setup:
#   imports = [ qpad.nixosModules.default ];
#   mordrag.services.qpad.enable = true;
#
# The qpad user and group are created automatically.  The service user is
# added to the "uinput" group (created by hardware.uinput) so it can write
# to /dev/uinput.
{ self, ... }:
{
  flake.nixosModules.default =
    {
      config,
      pkgs,
      lib,
      ...
    }:
    let
      inherit (pkgs.stdenv.hostPlatform) system;
      cfg = config.mordrag.services.qpad;
      qpad = self.packages.${system}.default;

      # Wrapper script placed in systemPackages so every user on the machine
      # can run the launcher without having to remember the port number.
      #
      #   qpad-launcher [/path/to/game]
      #
      # Additional arguments (e.g. --host) are passed through via "$@".
      launcherWrapper = pkgs.writeShellScriptBin "qpad-launcher" ''
        exec ${qpad}/bin/qpad-launcher --port ${toString cfg.port} "$@"
      '';
    in
    {
      options.mordrag.services.qpad = {
        enable = lib.mkEnableOption "qpad controller server";

        port = lib.mkOption {
          type = lib.types.port;
          description = ''
            TCP port the qpad web server listens on.
            Phones connecting via the QR code will reach this port.
          '';
        };

        openFirewall = lib.mkOption {
          type = lib.types.bool;
          default = false;
          description = "Open the configured port in the firewall.";
        };
      };

      config = lib.mkIf cfg.enable {

        # Delegate uinput setup to the upstream NixOS module, which:
        #   • loads the uinput kernel module
        #   • creates the "uinput" group
        #   • adds a udev rule: MODE="0660", GROUP="uinput",
        #     OPTIONS+="static_node=uinput"
        # The static_node option ensures /dev/uinput is present from boot
        # without requiring TAG+="systemd" or a separate device unit.
        hardware.uinput.enable = true;

        users = {
          users.qpad = {
            isSystemUser = true;
            group = "qpad";
            # "uinput" is the group created by hardware.uinput with access to
            # /dev/uinput via its udev rule.
            extraGroups = [ config.users.groups.uinput.name ];
            description = "qpad web server daemon";
          };
          groups.qpad = { };
        };

        systemd.services.qpad-web = {
          description = "qpad controller server and input bridge";

          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];

          serviceConfig = {
            User = config.users.users.qpad.name;
            Group = config.users.groups.qpad.name;
            SupplementaryGroups = [ config.users.groups.uinput.name ];

            ExecStart = "${qpad}/bin/qpad-web --port ${toString cfg.port}";

            Restart = "on-failure";
            RestartSec = "5s";

            # ── Hardening ──────────────────────────────────────────────────────
            # The service needs:
            #   • inbound network   (HTTP + WebSocket from phones)
            #   • /dev/uinput       (virtual gamepad injection)
            # Everything else is locked down.

            NoNewPrivileges = true;
            PrivateTmp = true;
            ProtectHome = true;
            ProtectClock = true;
            ProtectKernelTunables = true;
            ProtectKernelModules = true;
            ProtectKernelLogs = true;
            ProtectControlGroups = true;
            RestrictNamespaces = true;
            LockPersonality = true;
            MemoryDenyWriteExecute = true;
            RestrictRealtime = true;
            RestrictSUIDSGID = true;
            RemoveIPC = true;

            # Read-only filesystem except /tmp (PrivateTmp above).
            ProtectSystem = "strict";

            # Only allow access to input character devices.
            # "char-input rw" covers both /dev/uinput and /dev/input/event*.
            DevicePolicy = "closed";
            DeviceAllow = [
              "char-input rw"
              "/dev/uinput rw"
            ];

            # Syscall allow-list: standard service set plus ioctl (required
            # by the uinput kernel interface).
            SystemCallFilter = [
              "@system-service"
              "ioctl"
            ];
            SystemCallErrorNumber = "EPERM";
            SystemCallArchitectures = "native";
          };
        };

        environment.systemPackages = [ launcherWrapper ];
        programs.steam.extraPackages = [ launcherWrapper ];

        networking.firewall.allowedTCPPorts = lib.optionals cfg.openFirewall [ cfg.port ];
      };
    };
}
