//! Sidebar settings (global Sidebar tab): what the process-tree sidebar shows — the filter input,
//! empty-section hiding, the per-header CPU/memory usage thresholds, the project hover actions, and
//! the settings footer.
//!
//! Each usage threshold is a closed enum (the discrete options the demo offers), so a header shows
//! its CPU/memory badge only once usage reaches the chosen level. The option sets differ between
//! project and process headers, so each is its own enum — never a bare percentage or byte count.
//! The frontend maps each variant to its display label and comparison value.

use serde::{Deserialize, Serialize};

/// When a project header shows its CPU-usage badge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCpuThreshold {
    #[default]
    Always,
    Pct25,
    Pct50,
    Pct100,
    Pct200,
    Never,
}

/// When a project header shows its memory-usage badge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMemThreshold {
    #[default]
    Always,
    Mb500,
    Gb1,
    Gb2,
    Gb8,
    Never,
}

/// When a process header shows its CPU-usage badge.
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

/// When a process header shows its memory-usage badge.
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
    /// When a project header shows its CPU usage.
    pub project_cpu_threshold: ProjectCpuThreshold,
    /// When a project header shows its memory usage.
    pub project_mem_threshold: ProjectMemThreshold,
    /// Offer the "open in editor" hover action on a project header.
    pub project_open_in_editor: bool,
    /// Offer the "open in terminal" hover action on a project header.
    pub project_open_in_terminal: bool,
    /// Offer the "show in file manager" hover action on a project header.
    pub project_reveal_in_file_manager: bool,
    /// When a process header shows its CPU usage.
    pub process_cpu_threshold: ProcessCpuThreshold,
    /// When a process header shows its memory usage.
    pub process_mem_threshold: ProcessMemThreshold,
    /// Show the Settings button at the bottom of the sidebar (still reachable via palette and hotkey).
    pub show_settings_footer: bool,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            show_filter_input: true,
            hide_empty_sections: false,
            project_cpu_threshold: ProjectCpuThreshold::default(),
            project_mem_threshold: ProjectMemThreshold::default(),
            project_open_in_editor: true,
            project_open_in_terminal: true,
            project_reveal_in_file_manager: true,
            process_cpu_threshold: ProcessCpuThreshold::default(),
            process_mem_threshold: ProcessMemThreshold::default(),
            show_settings_footer: true,
        }
    }
}
