let 
  pkgs = import <nixpkgs> { };
  packages = with pkgs; [
    pkg-config
    udev
    alsa-lib
    gtk3
    wayland
    libxkbcommon
    libGL
    trunk
    cargo-edit
  ];
in
  pkgs.mkShell {
    buildInputs = packages;
    LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath packages}";
    RUST_BACKTRACE = 1;
    RUST_LOG = "info";
  }