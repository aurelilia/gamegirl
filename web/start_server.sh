#!/usr/bin/env sh
set -eu
echo "open http://localhost:8080"
python3 -m http.server 8080 --bind 127.0.0.1
