// One place for how long the harness is willing to wait. Named for what is being waited on, so
// neither a spec nor the runner config carries a bare number and a slow machine is tuned here
// rather than spec by spec. Every wait is on observable state — a rendered element, a settled
// value. Never a sleep.
export const WAIT = {
  /** A render the app does locally: an overlay opening, a row appearing. */
  render: 10_000,
  /** A round trip through the real core: loading a project, registering and starting a process. */
  core: 30_000,
  /**
   * Any WebDriver request to the app's embedded driver, retries included — session creation is the
   * slowest, so this is sized for it. The app's own startup is bounded separately, by the service's
   * `startTimeout` (60 s for the embedded provider, "the embedded WebDriver server takes longer to
   * come up"), which stays at its default: the binary is already built by `onPrepare`, so launching
   * it is a matter of seconds.
   */
  session: 120_000,
  /** One spec end to end — the mocha per-test ceiling, spanning several core round trips. */
  spec: 120_000,
} as const;
