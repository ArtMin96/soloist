//! Sidebar settings (global Sidebar tab): what the process-tree sidebar shows — the filter input,
//! empty-section hiding, the per-process CPU/memory usage thresholds, and the settings footer.
//!
//! Each usage threshold is a closed enum (the discrete options the demo offers), so a process row
//! shows its CPU/memory read-out only once usage reaches the chosen level — never a bare percentage
//! or byte count. The frontend maps each variant to its display label and comparison value.

use serde::{Deserialize, Serialize};

/// When a process row shows its CPU-usage read-out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessCpuThreshold {
    #[default]
    Always,
    Pct10,
    Pct30,
    Pct60,
    Pct90,
    Never,
}

/// When a process row shows its memory-usage read-out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessMemThreshold {
    #[default]
    Always,
    Mb100,
    Mb500,
    Gb1,
    Gb2,
    Never,
}

/// The Sidebar tab document. Every field carries a serde default so an older record still reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sidebar {
    /// Show the filter input at the top of the sidebar.
    pub show_filter_input: bool,
    /// Hide sections with no processes (e.g. Agents, Terminals).
    pub hide_empty_sections: bool,
    /// When a process row shows its CPU usage.
    pub process_cpu_threshold: ProcessCpuThreshold,
    /// When a process row shows its memory usage.
    pub process_mem_threshold: ProcessMemThreshold,
    /// Show the Settings button at the bottom of the sidebar (still reachable via palette and hotkey).
    pub show_settings_footer: bool,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            show_filter_input: true,
            hide_empty_sections: false,
            process_cpu_threshold: ProcessCpuThreshold::default(),
            process_mem_threshold: ProcessMemThreshold::default(),
            show_settings_footer: true,
        }
    }
}
