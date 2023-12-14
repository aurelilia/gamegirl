let 
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  pkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  rustChannel = (pkgs.rustChannelOf { rustToolchain = ./rust-toolchain.toml; });
  packages = with pkgs; [
    pkg-config
    udev
    alsa-lib
    gtk3
    rustChannel.rust
    rustChannel.rust-src
  ];
in
  pkgs.mkShell {
    buildInputs = packages;
    shellHook = ''
      export LD_LIBRARY_PATH="/usr/lib:${pkgs.lib.makeLibraryPath packages}:''${LD_LIBRARY_PATH}"	
      export RUST_SRC_PATH="${rustChannel.rust-src}/lib/rustlib/src/rust/library"
    '';

    RUST_BACKTRACE = 1;
    RUST_LOG = "info";
  }
