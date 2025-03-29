let
  pkgs = import <nixpkgs> { };
  packages = with pkgs; [
  	# Build related
    pkg-config
    udev
    llvmPackages.bintools
    clang
    alsa-lib
    gtk3
    gtk4
    wayland
    libxkbcommon
    libGL
    trunk

    # Useful tools
    cargo-llvm-lines
    cargo-bloat
    cargo-edit
    cargo-flamegraph
    cargo-watch
    gdb

    # Other emulators for ease of debugging
    mgba
  ];
in
  pkgs.mkShell {
    buildInputs = packages;
    LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath packages}";
    RUST_BACKTRACE = 1;
    RUST_LOG = "info";
  }
