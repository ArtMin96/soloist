// React Doctor scan configuration.
//
// `ignore.files` scopes the health score to product code: `src/components/ui` is vendored shadcn
// (also excluded in .prettierignore) and `src/harness.tsx` is the dev-only screenshot gallery whose
// code never ships (the Tauri/Vite build input is index.html alone).
//
// `ignore.overrides` records findings that were reviewed against the code (and tests) and confirmed
// as false positives or intentional patterns — each with the reason it is not a real defect. Keeping
// them here rather than editing the source leaves behaviour untouched and the decision auditable.
//
// Docs: https://react.doctor/docs
export default {
  ignore: {
    files: ["src/components/ui/**", "src/harness.tsx"],
    overrides: [
      {
        // The cleanup deliberately reads the LATEST ref to cancel the live PTY attachment and clear
        // the pending bell-linger timer. The rule's "capture the ref at effect start" fix would
        // capture null/undefined and never run — leaking the timer and skipping the detach. Verified
        // by useTerminal.test.tsx ("detaches with the token of its own attachment", "never writes a
        // superseded attachment's bytes").
        files: [
          "src/components/terminal/useTerminal.ts",
          "src/components/terminal/useTerminalChrome.ts",
        ],
        rules: ["react-doctor/exhaustive-deps"],
      },
      {
        // These effects react to EXTERNAL backend `ProcessStatusChanged` domain events (routed
        // through useProcesses), not a local event handler — there is nowhere to "move the handler".
        // Line 203 is a guarded imperative attach(), not derived state; the attach lifecycle
        // (attaching/live/not-started) is orthogonal to process.status.
        files: ["src/components/terminal/useTerminal.ts"],
        rules: ["react-doctor/no-event-handler", "react-doctor/no-derived-state"],
      },
      {
        // `input.indexOf(char, i + 1)` is a positional forward-search for a matching closing quote,
        // not a repeated membership test — a Set cannot answer "is there a closing quote later", and
        // the input is a short one-shot CLI string.
        files: ["src/lib/tokenizeArgs.ts"],
        rules: ["react-doctor/js-set-map-lookups"],
      },
      {
        // A read-only display list (bullet points) that never reorders or filters — the row index is
        // a stable key here.
        files: ["src/components/orchestration/TodoItem.tsx"],
        rules: ["react-doctor/no-array-index-as-key"],
      },
      {
        // A correct WAI-ARIA roving-tabindex tablist: focus lives on the child `role="tab"` buttons
        // (tabIndex 0/-1) and the container is intentionally not itself a tab stop. Adding tabIndex
        // to the tablist would create a wrong extra tab stop.
        files: ["src/components/settings/SettingsTabRail.tsx"],
        rules: ["react-doctor/interactive-supports-focus"],
      },
      {
        // The initial value is not synchronously knowable: it comes from an async Tauri call
        // (isWindowMaximized) or a not-yet-attached DOM ref (scrollTop), so it cannot seed useState;
        // the effect also subscribes to live updates, so it is not a pure mount-time initializer.
        files: ["src/components/titlebar/useWindowControls.ts", "src/store/useScrollEdge.ts"],
        rules: ["react-doctor/no-initialize-state"],
      },
      {
        // The <nav> is a navigation landmark hosting the sidebar's ARIA tree (role="tree"/"treeitem",
        // each row focusable with Enter/Space). Its onKeyDown is a catch-all surface for the
        // user-configurable sidebar hotkeys (jump-to-section, collapse, restart) that fire on events
        // bubbling from any focused child — not a widget interaction, and there is no single
        // interactive role to add to a landmark that contains multiple trees.
        files: ["src/components/sidebar/Sidebar.tsx"],
        rules: ["react-doctor/no-noninteractive-element-interactions"],
      },
      {
        // cmdk's <CommandGroup heading> requires a composed ReactNode (a status dot + label) built
        // per row inside a .map(), so it cannot be hoisted or useMemo'd; cmdk's group is not memoized,
        // so the fresh element carries no re-render cost.
        files: ["src/components/QuickActionsPalette.tsx"],
        rules: ["react-doctor/jsx-no-jsx-as-prop"],
      },
      {
        // Eight independent form fields with no cross-field invariants; the only shared transition is
        // reset(). A useReducer would add per-field action indirection without improving consistency,
        // so plain useState per field is clearer here.
        files: ["src/components/project-settings/AddCommandModal.tsx"],
        rules: ["react-doctor/prefer-useReducer"],
      },
    ],
  },
};
