#!/bin/sh

cargo build -p dynacore --release
cp $CARGO_TARGET_DIR/release/libdynacore.so dyn-cores/$1
