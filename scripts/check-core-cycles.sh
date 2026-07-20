#!/usr/bin/env bash
# Enforces the acyclic-context rule: the core's modules form a DAG, so a context can be read,
# tested, and changed without dragging in a ring of its neighbours.
#
# This is a gate rather than a doc because the doc did not hold: the architecture claimed "no
# cycles between contexts" while the port layer imported concrete Noop adapters back from nine
# contexts that imported it in turn. Nothing checked the claim, so it quietly stopped being true.
#
# There is no allow-list. A value type shared by several contexts belongs in a shared-kernel module
# that depends on nothing (`process`, `idle`, `configchange`, `ids`), not in whichever context feels
# closest to it — that is what keeps the graph acyclic without exceptions.
#
# Edges come from `use crate::<module>` in production code only — a file's inline `#[cfg(test)]`
# module is cut first (as check-file-size.sh does), and dedicated test files are skipped, because
# a test reaching across the core is not a design cycle.
set -uo pipefail

cd "$(dirname "$0")/.."

mapfile -t files < <(git ls-files 'crates/core/src/*.rs' \
  | grep -vE '_tests\.rs$' \
  | grep -vE '(^|/)test_support\.rs$' \
  | grep -vE '(^|/)testing/')

# "module -> module" edges, deduped. A file's module is its first path component under src/
# (`coordination/todo.rs` -> `coordination`, `ports.rs` -> `ports`); an import of a module's own
# name is not an edge. `testing` is test-only scaffolding and never a node.
edges() {
  local f from
  for f in "${files[@]}"; do
    [ -f "$f" ] || continue
    from="${f#crates/core/src/}"
    from="${from%%/*}"
    from="${from%.rs}"
    # POSIX awk only: the capture-array form of `match` is a gawk extension, and under mawk it is a
    # syntax error that yields no edges at all — which reads as a clean graph. RSTART/RLENGTH and
    # `sub` are portable and say the same thing.
    awk -v from="$from" '
      /^[[:space:]]*#\[cfg\(test\)\]/ { exit }
      match($0, /^[[:space:]]*use crate::[a-z_][a-z0-9_]*/) {
        mod = substr($0, RSTART, RLENGTH)
        sub(/^[[:space:]]*use crate::/, "", mod)
        if (mod != from && mod != "testing") print from " " mod
      }
    ' "$f"
  done | sort -u
}

mapfile -t graph < <(edges)

# An empty graph is not a clean graph. The core's modules always import across each other, so zero
# edges means extraction broke — a stricter awk, a moved source root, a renamed glob — and an empty
# edge list trivially has no cycles. Without this the gate reports success while checking nothing.
if [ "${#graph[@]}" -eq 0 ]; then
  echo "error: no module edges found, so nothing was actually checked."
  echo "The core always imports across its modules; zero edges means the extraction above is"
  echo "broken rather than the graph being acyclic. Check the awk invocation and the file glob."
  exit 1
fi

# Report every cycle by walking each edge back to its source (depth-first over the edge list).
cycles="$(printf '%s\n' "${graph[@]}" | awk '
  { succ[$1] = succ[$1] " " $2; nodes[$1]; nodes[$2] }
  END {
    for (n in nodes) {
      # Seed a walk at n, tracking the path so a return to n prints the ring.
      split("", seen); split("", stack)
      head = 0; tail = 0
      stack[tail++] = n; path[n] = n
      while (head < tail) {
        cur = stack[head++]
        split(succ[cur], next_, " ")
        for (i in next_) {
          nxt = next_[i]
          if (nxt == "") continue
          if (nxt == n) { print path[cur] " -> " n; delete nodes[n]; break }
          if (!(nxt in seen)) { seen[nxt]; stack[tail++] = nxt; path[nxt] = path[cur] " -> " nxt }
        }
      }
    }
  }
' | sort -u)"

if [ -z "$cycles" ]; then
  echo "core-cycles OK: ${#graph[@]} module edges, no cycles"
  exit 0
fi

echo "error: import cycles between soloist-core modules:"
printf '%s\n' "$cycles" | sed 's/^/  /'
echo
echo "A context must not depend on something that depends on it. Two shapes fix most of these:"
echo "  - a port belongs with the context that drives it, not in a module every context imports;"
echo "  - only the composition root (crate::composition) may name every context at once."
exit 1
