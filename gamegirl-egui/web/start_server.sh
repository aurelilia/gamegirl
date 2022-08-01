#!/usr/bin/env sh
#
# Unless otherwise noted, this file is released and thus subject to the
# terms of the Mozilla Public License Version 2.0 (MPL2). Also, it is
# "Incompatible With Secondary Licenses", as defined by the MPL2.
# If a copy of the MPL2 was not distributed with this file, you can
# obtain one at https://mozilla.org/MPL/2.0/.
#

set -eu
echo "open http://localhost:8080"
python3 -m http.server 8080 --bind 127.0.0.1
