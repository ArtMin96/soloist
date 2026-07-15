// The Sidebar tab's pre-load default, the process-threshold option sets, and the usage floor each
// threshold implies. Each threshold is a closed enum (mirrors core); the panel maps a variant to
// its display label, and a process row shows its CPU/memory read-out only once usage reaches the
// mapped floor — both mappings live here, in one place.

import type { Option } from "@/lib/appearance";
import type { ProcessCpuThreshold, ProcessMemThreshold, Sidebar } from "@/domain";

// Mirrors core::Sidebar::default(): filter on, empty sections shown, usage always shown, footer on.
// The facade's stored value supersedes this on load.
export const DEFAULT_SIDEBAR: Sidebar = {
  show_filter_input: true,
  hide_empty_sections: false,
  process_cpu_threshold: "always",
  process_mem_threshold: "always",
  show_settings_footer: true,
};

export const PROCESS_CPU_OPTIONS: Option<ProcessCpuThreshold>[] = [
  { value: "always", label: "Always" },
  { value: "pct10", label: "10%" },
  { value: "pct30", label: "30%" },
  { value: "pct60", label: "60%" },
  { value: "pct90", label: "90%" },
  { value: "never", label: "Never" },
];

export const PROCESS_MEM_OPTIONS: Option<ProcessMemThreshold>[] = [
  { value: "always", label: "Always" },
  { value: "mb100", label: "100 MB" },
  { value: "mb500", label: "500 MB" },
  { value: "gb1", label: "1 GB" },
  { value: "gb2", label: "2 GB" },
  { value: "never", label: "Never" },
];

const MIB = 1024 * 1024;
const GIB = 1024 * MIB;

// The CPU-percent a process must reach for its row to show the CPU read-out. `always` shows at any
// usage (floor 0); `never` hides it (floor Infinity).
export const PROCESS_CPU_FLOOR: Record<ProcessCpuThreshold, number> = {
  always: 0,
  pct10: 10,
  pct30: 30,
  pct60: 60,
  pct90: 90,
  never: Infinity,
};

// The resident bytes a process must reach for its row to show the memory read-out.
export const PROCESS_MEM_FLOOR: Record<ProcessMemThreshold, number> = {
  always: 0,
  mb100: 100 * MIB,
  mb500: 500 * MIB,
  gb1: GIB,
  gb2: 2 * GIB,
  never: Infinity,
};
