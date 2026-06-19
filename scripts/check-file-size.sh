#!/usr/bin/env bash
# Advisory signal for the ~400 non-test-line "split smell" (CLAUDE.md §15, plan/06 §7).
# Reports source files whose CODE size crosses the threshold so a split (e.g. R2) is a
# deliberate decision, not an oversight. It measures code, not test bloat: dedicated test
# files (integration `tests/`, `*.test.ts(x)`) are skipped, and a Rust file's inline
# `#[cfg(test)]` module is excluded from the count.
#
# Warn-only by design: it always exits 0 so it never gates the build. Tightening it into a
# hard gate is a later, separate decision (plan/06 §9), the way check-core-deps.sh gates
# layering today.
set -uo pipefail

THRESHOLD="${FILE_SIZE_THRESHOLD:-400}"

# Tracked sources only (git already ignores target/, node_modules/, dist/), minus test
# files: integration tests live under a `tests/` dir, unit tests are `*.test.ts(x)`.
mapfile -t files < <(git ls-files '*.rs' '*.ts' '*.tsx' \
  | grep -vE '(^|/)tests/' \
  | grep -vE '\.test\.(ts|tsx)$')

# Non-test line count: for Rust, everything before the first `#[cfg(test)]` attribute
# (the inline test module marker); whole file if absent. Other sources count in full.
nontest_lines() {
  local f="$1"
  case "$f" in
    *.rs)
      awk '/^[[:space:]]*#\[cfg\(test\)\]/ { print NR - 1; found = 1; exit }
           END { if (!found) print NR }' "$f"
      ;;
    *)
      awk 'END { print NR }' "$f"
      ;;
  esac
}

outliers=()
for f in "${files[@]}"; do
  [ -f "$f" ] || continue
  n="$(nontest_lines "$f")"
  if [ "${n:-0}" -gt "$THRESHOLD" ]; then
    outliers+=("$n	$f")
  fi
done

if [ "${#outliers[@]}" -eq 0 ]; then
  echo "file-size OK: no source file exceeds ${THRESHOLD} non-test lines"
  exit 0
fi

echo "file-size warning: ${#outliers[@]} source file(s) over the ${THRESHOLD} non-test-line split smell (advisory):"
printf '%s\n' "${outliers[@]}" | sort -rn | while IFS=$'\t' read -r n f; do
  printf '  %5d  %s\n' "$n" "$f"
done
echo "  (non-gating — see plan/06 §7 for the split roadmap)"
exit 0
