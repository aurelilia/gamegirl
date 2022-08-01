#!/usr/bin/env sh
#
# Unless otherwise noted, this file is released and thus subject to the
# terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
# "Incompatible With Secondary Licenses", as defined by the MPL2.
# If a copy of the MPL2 was not distributed with this file, you can
# obtain one at https://mozilla.org/MPL/2.0/.
#

set -eu
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
cargo update -p wasm-bindgen
