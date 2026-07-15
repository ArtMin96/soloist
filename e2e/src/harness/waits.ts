// One place for how long the harness is willing to wait. Named for what is being waited on, so a
// spec never carries a bare number and a slow machine is tuned here rather than spec by spec.
// Every wait is on observable state — a rendered element, a settled value. Never a sleep.
export const WAIT = {
  /** A render the app does locally: an overlay opening, a row appearing. */
  render: 10_000,
  /** A round trip through the real core: loading a project, registering and starting a process. */
  core: 30_000,
} as const;
