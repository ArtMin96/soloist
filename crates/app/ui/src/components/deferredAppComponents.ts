import { lazy } from "react";

// Main-area panes and overlays each load their own chunk on first use. Keeping the declarations
// together leaves App responsible for composition while the eager shell and safety dialogs stay in
// its initial bundle.
export const TerminalPane = lazy(() =>
  import("@/components/terminal/TerminalPane").then((module) => ({
    default: module.TerminalPane,
  })),
);
export const ProjectSettingsPane = lazy(() =>
  import("@/components/project-settings/ProjectSettingsPane").then((module) => ({
    default: module.ProjectSettingsPane,
  })),
);
export const OrchestrationPane = lazy(() =>
  import("@/components/orchestration/OrchestrationPane").then((module) => ({
    default: module.OrchestrationPane,
  })),
);
export const SettingsOverlay = lazy(() =>
  import("@/components/settings/SettingsOverlay").then((module) => ({
    default: module.SettingsOverlay,
  })),
);
export const LaunchPicker = lazy(() =>
  import("@/components/LaunchPicker").then((module) => ({ default: module.LaunchPicker })),
);
export const QuickJumpPalette = lazy(() =>
  import("@/components/QuickJumpPalette").then((module) => ({
    default: module.QuickJumpPalette,
  })),
);
export const QuickActionsPalette = lazy(() =>
  import("@/components/QuickActionsPalette").then((module) => ({
    default: module.QuickActionsPalette,
  })),
);
export const CommandPalette = lazy(() =>
  import("@/components/CommandPalette").then((module) => ({
    default: module.CommandPalette,
  })),
);
// The version-control rail and everything it needs — the tree engine included — load together,
// so an app opened on a project without version control never pays for them.
export const GitRail = lazy(() =>
  import("@/components/git/GitRail").then((module) => ({ default: module.GitRail })),
);
