#!/bin/sh

cargo build -p gamegirl --release --features dynamic,gga,nds
cp $CARGO_TARGET_DIR/release/libgamegirl.so dyn-cores/$1
