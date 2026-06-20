import type { ProcessKind } from "@/domain";

// The single-letter monogram for a project's avatar fallback when it has no icon: the first
// character of its name, uppercased ("?" for an empty name). The avatar renders the project's
// icon when one is present and falls back to this otherwise.
export function monogram(name: string): string {
  return name.trim().charAt(0).toUpperCase() || "?";
}

// Stable collapse-state keys for the sidebar tree: one per project node, one per kind subgroup
// within a project. Defined here so the sidebar consumes the keys rather than formatting them
// itself — the key shape lives in the projects module, not in the component.
export function projectCollapseKey(id: number): string {
  return `project:${id}`;
}

export function kindCollapseKey(id: number, kind: ProcessKind): string {
  return `kind:${id}:${kind}`;
}
