#!/usr/bin/env sh
#
# Unless otherwise noted, this file is released and thus subject to the
# terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
# "Incompatible With Secondary Licenses", as defined by the MPL2.
# If a copy of the MPL2 was not distributed with this file, you can
# obtain one at https://mozilla.org/MPL/2.0/.
#

set -eu
cd ..

OPEN=false
FAST=false

while test $# -gt 0; do
  case "$1" in
    -h|--help)
      echo "build_web.sh [--fast] [--open]"
      echo "  --fast: skip optimization step"
      echo "  --open: open the result in a browser"
      exit 0
      ;;
    --fast)
      shift
      FAST=true
      ;;
    --open)
      shift
      OPEN=true
      ;;
    *)
      break
      ;;
  esac
done

# ./setup_web.sh # <- call this first!
CRATE_NAME="gamegirl-egui"
CRATE_NAME_SNAKE_CASE="gamegirl_egui"

# This is required to enable the web_sys clipboard API which egui_web uses
# https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.Clipboard.html
# https://rustwasm.github.io/docs/wasm-bindgen/web-sys/unstable-apis.html
export RUSTFLAGS=--cfg=web_sys_unstable_apis

# Clear output from old stuff:
rm -f "web/gamegirl_bg.wasm"

echo "Building rust…"
BUILD=release
cargo build -p "gamegirl-egui" --release --lib --target wasm32-unknown-unknown

# Get the output directory (in the workspace it is in another location)
TARGET=$(cargo metadata --format-version=1 | jq --raw-output .target_directory)

echo "Generating JS bindings for wasm…"
TARGET_NAME="${CRATE_NAME_SNAKE_CASE}.wasm"
WASM_PATH="${TARGET}/wasm32-unknown-unknown/${BUILD}/${TARGET_NAME}"
wasm-bindgen "${WASM_PATH}" --out-dir web --no-modules --no-typescript

if [[ "${FAST}" == false ]]; then
  echo "Optimizing wasm…"
  # to get wasm-opt:  apt/brew/dnf install binaryen
  wasm-opt "docs/${CRATE_NAME}_bg.wasm" -O2 --fast-math -o "docs/${CRATE_NAME}_bg.wasm" # add -g to get debug symbols
fi

echo "Finished: web/${CRATE_NAME_SNAKE_CASE}.wasm"

if [[ "${OPEN}" == true ]]; then
  xdg-open http://localhost:8080/index.html
fi
