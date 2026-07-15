// One place for how long the harness is willing to wait. Named for what is being waited on, so
// neither a spec nor the runner config carries a bare number and a slow machine is tuned here
// rather than spec by spec. Every wait is on observable state — a rendered element, a settled
// value. Never a sleep.
export const WAIT = {
  /** A render the app does locally: an overlay opening, a row appearing. */
  render: 10_000,
  /** A round trip through the real core: loading a project, registering and starting a process. */
  core: 30_000,
  /** Launching the built app and accepting the WebDriver session, retries included. */
  session: 120_000,
  /** One spec end to end — the mocha per-test ceiling, spanning several core round trips. */
  spec: 120_000,
} as const;
