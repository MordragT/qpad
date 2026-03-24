{
  perSystem =
    { inputs', pkgs, ... }:
    {
      packages.default =
        (pkgs.makeRustPlatform {
          inherit (inputs'.fenix.packages.complete) cargo rustc;
        }).buildRustPackage
          {
            pname = "qpad";
            version = "0.1.0";
            src = ../.;
            cargoLock.lockFile = ../Cargo.lock;
          };
    };
}
