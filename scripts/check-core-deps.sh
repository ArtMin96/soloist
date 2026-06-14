#!/usr/bin/env bash
# Enforces the hexagonal dependency rule: the domain core must never depend on an
# adapter framework. Adapters depend on the core, never the reverse.
set -euo pipefail

FORBIDDEN=(tauri rmcp axum rusqlite notify-rust)
tree="$(cargo tree -p soloist-core --prefix none --no-dedupe)"
names="$(printf '%s\n' "$tree" | awk '{print $1}')"

status=0
# Match the crate itself and any sub-crate (e.g. `tauri-utils`, `axum-core`), so a
# transitive piece of a forbidden framework can't slip past an exact-name check.
for crate in "${FORBIDDEN[@]}"; do
  if printf '%s\n' "$names" | grep -qE "^${crate}(-.*)?$"; then
    echo "error: soloist-core must not depend on '$crate' (or a sub-crate of it)"
    status=1
  fi
done

if [ "$status" -eq 0 ]; then
  echo "dependency-direction OK: soloist-core is framework-free"
fi
exit "$status"
