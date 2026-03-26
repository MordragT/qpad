{
  perSystem =
    {
      inputs',
      pkgs,
      lib,
      ...
    }:
    let
      toolchain = inputs'.fenix.packages.complete;
    in
    {
      devShells.default =
        let
          packages =
            (with toolchain; [
              cargo
              rustc
              rust-src
              clippy
              rustfmt
            ])
            ++ (with pkgs; [
              pkg-config
              nixfmt
              wayland
              wayland-protocols
              libxkbcommon
              libGL
            ]);

        in
        pkgs.mkShell {
          inherit packages;

          # Specify the rust-src path (many editors rely on this)
          env = {
            LD_LIBRARY_PATH = lib.makeLibraryPath packages;
            RUST_SRC_PATH = "${toolchain.rust-src}/lib/rustlib/src/rust/library";
          };
        };
    };
}
