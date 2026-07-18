/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" in the WebDriver end-to-end build (set by the e2e harness); undefined in a normal build. */
  readonly VITE_E2E?: string;
}
