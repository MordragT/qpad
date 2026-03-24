{
  perSystem =
    { inputs', pkgs, ... }:
    let
      toolchain = inputs'.fenix.packages.complete;
    in
    {
      devShells.default = pkgs.mkShell {
        # Use nightly cargo & rustc provided by fenix. Add for packages for the dev shell here
        buildInputs = with pkgs; [
          (with toolchain; [
            cargo
            rustc
            rust-src
            clippy
            rustfmt
          ])
          pkg-config
          nixfmt
        ];

        # Specify the rust-src path (many editors rely on this)
        RUST_SRC_PATH = "${toolchain.rust-src}/lib/rustlib/src/rust/library";
      };
    };
}
