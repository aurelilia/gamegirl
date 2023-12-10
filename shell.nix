let 
  pkgs = import <nixpkgs> {};
  packages = with pkgs; [
    pkg-config
    udev
    alsa-lib
    gtk3
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
