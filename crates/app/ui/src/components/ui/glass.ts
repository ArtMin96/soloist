// Shared glass treatments. Every glass surface draws a bevel — a light rim on top over a soft
// shadow below, authored in runtime.ts's `--glass-*-shadow` tokens — so it reads as lit frosted
// glass rather than a flat panel. The bevel itself is never gated behind `supports-backdrop-filter`:
// losing it entirely on an engine without real blur is a harder failure than losing translucency.
// Only the tint, the border tone, and the blur/saturation are conditional on that support.

export const GLASS_FLOATING_SURFACE =
  "border border-border bg-popover shadow-overlay supports-backdrop-filter:border-[var(--glass-border)] supports-backdrop-filter:bg-[var(--glass-surface)] supports-backdrop-filter:[box-shadow:var(--glass-floating-shadow)] supports-backdrop-filter:backdrop-blur-xl supports-backdrop-filter:backdrop-saturate-150";

// Modal dialogs only — the same glass treatment as GLASS_FLOATING_SURFACE, one shadow weight
// heavier, for the surface that owns the whole viewport's focus.
export const GLASS_MODAL_SURFACE =
  "border border-border bg-popover shadow-dialog supports-backdrop-filter:border-[var(--glass-border)] supports-backdrop-filter:bg-[var(--glass-surface)] supports-backdrop-filter:[box-shadow:var(--glass-floating-shadow)] supports-backdrop-filter:backdrop-blur-xl supports-backdrop-filter:backdrop-saturate-150";

export const GLASS_CONTROL_SURFACE =
  "border-input bg-toolbar-control [box-shadow:var(--glass-control-shadow)] supports-backdrop-filter:border-[var(--glass-border)] supports-backdrop-filter:bg-[var(--glass-control-surface)] supports-backdrop-filter:backdrop-blur-md supports-backdrop-filter:backdrop-saturate-150";

export const GLASS_INTERACTIVE_CONTROL_SURFACE = `${GLASS_CONTROL_SURFACE} hover:bg-toolbar-control-hover aria-expanded:bg-toolbar-control-hover data-[state=open]:bg-toolbar-control-hover supports-backdrop-filter:hover:bg-[var(--glass-control-hover)] supports-backdrop-filter:aria-expanded:bg-[var(--glass-control-active)] supports-backdrop-filter:data-[state=open]:bg-[var(--glass-control-active)]`;

export const GLASS_GHOST_INTERACTION =
  "hover:border-[var(--glass-border)] hover:bg-toolbar-control-hover hover:[box-shadow:var(--glass-control-shadow)] aria-expanded:border-[var(--glass-border)] aria-expanded:bg-toolbar-control-hover aria-expanded:[box-shadow:var(--glass-control-shadow)] supports-backdrop-filter:hover:bg-[var(--glass-control-hover)] supports-backdrop-filter:hover:backdrop-blur-md supports-backdrop-filter:hover:backdrop-saturate-150 supports-backdrop-filter:aria-expanded:bg-[var(--glass-control-active)] supports-backdrop-filter:aria-expanded:backdrop-blur-md supports-backdrop-filter:aria-expanded:backdrop-saturate-150";
