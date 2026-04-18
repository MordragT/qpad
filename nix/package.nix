{
  perSystem =
    {
      pkgs,
      lib,
      ...
    }:
    {
      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "qpad";
        version = "0.1.0";
        src = ../.;
        cargoLock.lockFile = ../Cargo.lock;

        # Building the whole workspace produces both qpad-web and qpad-launcher.
        # No cargoBuildFlags needed — cargo builds all workspace members by default.

        # Tests require a live system (uinput device, network) — skip in sandbox.
        doCheck = false;

        nativeBuildInputs = with pkgs; [
          pkg-config
          makeWrapper
        ];

        # GUI libs are needed at compile time for qpad-launcher (eframe/egui).
        # qpad-web is headless and ignores them.
        buildInputs = with pkgs; [
          vulkan-loader
          wayland
          wayland-protocols
          libxkbcommon
          libGL
          # X11 fallback (winit uses X11 when Wayland is unavailable)
          libx11
          libxi
          libxrandr
          libxcursor
        ];

        meta = {
          description = "qpad — gamepad-over-WiFi controller server and launcher";
          license = lib.licenses.mit;
          platforms = lib.platforms.linux;
          mainProgram = "qpad-web";
        };
      };
    };
}
