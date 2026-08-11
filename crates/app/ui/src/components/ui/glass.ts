export const GLASS_FLOATING_SURFACE =
  "border-border bg-popover shadow-overlay supports-backdrop-filter:border-[var(--glass-border)] supports-backdrop-filter:bg-[var(--glass-surface)] supports-backdrop-filter:[box-shadow:var(--glass-floating-shadow)] supports-backdrop-filter:backdrop-blur-xl supports-backdrop-filter:backdrop-saturate-150";

export const GLASS_CONTROL_SURFACE =
  "border-input bg-toolbar-control [box-shadow:var(--glass-control-shadow)] supports-backdrop-filter:border-[var(--glass-border)] supports-backdrop-filter:bg-[var(--glass-control-surface)] supports-backdrop-filter:backdrop-blur-md supports-backdrop-filter:backdrop-saturate-150";

export const GLASS_INTERACTIVE_CONTROL_SURFACE = `${GLASS_CONTROL_SURFACE} hover:bg-toolbar-control-hover aria-expanded:bg-toolbar-control-hover data-[state=open]:bg-toolbar-control-hover supports-backdrop-filter:hover:bg-[var(--glass-control-hover)] supports-backdrop-filter:aria-expanded:bg-[var(--glass-control-active)] supports-backdrop-filter:data-[state=open]:bg-[var(--glass-control-active)]`;

export const GLASS_GHOST_INTERACTION =
  "hover:border-[var(--glass-border)] hover:bg-toolbar-control-hover hover:[box-shadow:var(--glass-control-shadow)] aria-expanded:border-[var(--glass-border)] aria-expanded:bg-toolbar-control-hover aria-expanded:[box-shadow:var(--glass-control-shadow)] supports-backdrop-filter:hover:bg-[var(--glass-control-hover)] supports-backdrop-filter:hover:backdrop-blur-md supports-backdrop-filter:hover:backdrop-saturate-150 supports-backdrop-filter:aria-expanded:bg-[var(--glass-control-active)] supports-backdrop-filter:aria-expanded:backdrop-blur-md supports-backdrop-filter:aria-expanded:backdrop-saturate-150";
