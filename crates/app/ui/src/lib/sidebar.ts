// The Sidebar tab's pre-load default and the discrete threshold option sets. Each usage
// threshold is a closed enum (mirrors core); the panel maps a variant to its display label
// here, in one place. The comparison value each threshold implies belongs with the header
// usage badges, which are a later sidebar feature — only the labels are needed today.

import type { Option } from "@/lib/appearance";
import type {
  ProcessCpuThreshold,
  ProcessMemThreshold,
  ProjectCpuThreshold,
  ProjectMemThreshold,
  Sidebar,
} from "@/domain";

// Mirrors core::Sidebar::default(): filter on, empty sections shown, badges always, hover
// actions on, footer on. The facade's stored value supersedes this on load.
export const DEFAULT_SIDEBAR: Sidebar = {
  show_filter_input: true,
  hide_empty_sections: false,
  project_cpu_threshold: "always",
  project_mem_threshold: "always",
  project_open_in_editor: true,
  project_open_in_terminal: true,
  project_reveal_in_file_manager: true,
  process_cpu_threshold: "always",
  process_mem_threshold: "always",
  show_settings_footer: true,
};

export const PROJECT_CPU_OPTIONS: Option<ProjectCpuThreshold>[] = [
  { value: "always", label: "Always" },
  { value: "pct25", label: "25%" },
  { value: "pct50", label: "50%" },
  { value: "pct100", label: "100%" },
  { value: "pct200", label: "200%" },
  { value: "never", label: "Never" },
];

export const PROJECT_MEM_OPTIONS: Option<ProjectMemThreshold>[] = [
  { value: "always", label: "Always" },
  { value: "mb500", label: "500 MB" },
  { value: "gb1", label: "1 GB" },
  { value: "gb2", label: "2 GB" },
  { value: "gb8", label: "8 GB" },
  { value: "never", label: "Never" },
];

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
