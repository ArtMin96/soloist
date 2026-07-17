#!/usr/bin/env bash
# A long-lived stub that binds an ephemeral TCP port. The port is new on every spawn, which is
# what makes a *real* restart window-observable: the row's discovered port must change, so a
# restart that merely repainted the row cannot pass.
set -euo pipefail

exec python3 -m http.server 0 --bind 127.0.0.1
