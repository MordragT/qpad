{
  perSystem =
    {
      pkgs,
      lib,
      ...
    }:
    {
      devShells.default =
        let
          packages = with pkgs; [
            evtest
            cargo
            rustc
            rustfmt
            clippy
            pkg-config
            nixfmt
            wayland
            wayland-protocols
            libxkbcommon
            libGL
          ];

        in
        pkgs.mkShell {
          inherit packages;

          # Specify the rust-src path (many editors rely on this)
          env = {
            LD_LIBRARY_PATH = lib.makeLibraryPath packages;
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        };
    };
}
