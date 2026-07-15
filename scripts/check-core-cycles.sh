#!/usr/bin/env bash
# Enforces the acyclic-context rule: the core's modules form a DAG, so a context can be read,
# tested, and changed without dragging in a ring of its neighbours.
#
# This is a gate rather than a doc because the doc did not hold: the architecture claimed "no
# cycles between contexts" while the port layer imported concrete Noop adapters back from nine
# contexts that imported it in turn. Nothing checked the claim, so it quietly stopped being true.
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

# Known-accepted edges, ignored when detecting cycles. This is a ratchet, not an amnesty: it
# holds the line at today's debt so a *new* cycle fails the build, and every entry is a decision
# still owed.
#
# `DomainEvent` names the payload types it carries, and the contexts owning those types also
# publish to the bus, so events and those contexts import each other. Removing this needs the
# payload vocabulary to move to a shared kernel — the way `process` already holds ProcStatus for
# both events and the supervisor — which is a design decision, not a rename. Until then these two
# edges are the only cycles the core is allowed to have.
ALLOWED=(
  "events agents"   # DomainEvent carries AgentActivity; agents/idle/sampler publishes
  "events config"   # DomainEvent carries ConfigSync + TrustReviewCommand; config/sync publishes
)

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
    awk -v from="$from" '
      /^[[:space:]]*#\[cfg\(test\)\]/ { exit }
      match($0, /^[[:space:]]*use crate::([a-z_][a-z0-9_]*)/, m) {
        if (m[1] != from && m[1] != "testing") print from " " m[1]
      }
    ' "$f"
  done | sort -u
}

is_allowed() {
  local e
  for e in "${ALLOWED[@]}"; do [ "$e" = "$1" ] && return 0; done
  return 1
}

mapfile -t all_edges < <(edges)
graph=()
skipped=0
for e in "${all_edges[@]}"; do
  if is_allowed "$e"; then skipped=$((skipped + 1)); else graph+=("$e"); fi
done

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
  echo "core-cycles OK: ${#graph[@]} module edges, no cycles (${skipped} known edges allowed)"
  exit 0
fi

echo "error: import cycles between soloist-core modules:"
printf '%s\n' "$cycles" | sed 's/^/  /'
echo
echo "A context must not depend on something that depends on it. Two shapes fix most of these:"
echo "  - a port belongs with the context that drives it, not in a module every context imports;"
echo "  - only the composition root (crate::composition) may name every context at once."
exit 1
