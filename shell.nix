let 
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  pkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  packages = with pkgs; [
    pkg-config
    udev
    alsa-lib
    gtk3
    (rustChannelOf { rustToolchain = ./rust-toolchain.toml; }).rust
  ];
in
  pkgs.mkShell {
    buildInputs = packages;
    shellHook = ''
      export LD_LIBRARY_PATH="/usr/lib:${pkgs.lib.makeLibraryPath packages}:''${LD_LIBRARY_PATH}"	
    '';

    RUST_BACKTRACE = 1;
    RUST_LOG = "info";
  }
