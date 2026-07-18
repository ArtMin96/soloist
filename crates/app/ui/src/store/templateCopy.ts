// Picks a free name for a duplicated template: `<base> copy`, then `<base> copy 2`, `… 3`, … until
// one is not already taken. Pure so the manager's duplicate action stays a projection and the naming
// rule can be unit-tested on its own. Names are matched case-sensitively, mirroring the core's
// per-(kind, scope) uniqueness.
export function uniqueCopyName(base: string, existing: readonly string[]): string {
  const taken = new Set(existing);
  const first = `${base} copy`;
  if (!taken.has(first)) return first;
  for (let n = 2; ; n += 1) {
    const candidate = `${first} ${n}`;
    if (!taken.has(candidate)) return candidate;
  }
}
