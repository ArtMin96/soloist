import type { AgentActivity } from "@/domain";
import type { StatusDisplay } from "@/lib/status";

// The single source for turning an agent's activity into its display. It extends the same
// glyph + color + label vocabulary as ProcStatus (see lib/status), never hue alone, so an
// agent's live state reads at a glance and survives color blindness — not a parallel system.
// Working/Idle/Error reuse the running/stopped/crashed tokens whose meaning matches; Thinking
// reuses the transitional amber and pulses; Permission has its own attention token. The
// exhaustive Record requires an entry for every activity.
export const ACTIVITY: Record<AgentActivity, StatusDisplay> = {
  Working: { label: "Working", glyph: "▶", toneClass: "text-status-running", transitional: false },
  Thinking: {
    label: "Thinking",
    glyph: "◐",
    toneClass: "text-status-transition",
    transitional: true,
  },
  Idle: { label: "Idle", glyph: "○", toneClass: "text-status-stopped", transitional: false },
  Permission: {
    label: "Permission",
    glyph: "◆",
    toneClass: "text-status-attention",
    transitional: false,
  },
  Error: { label: "Error", glyph: "✕", toneClass: "text-status-crashed", transitional: false },
};
