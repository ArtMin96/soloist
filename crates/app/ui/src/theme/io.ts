import type { ThemeDefinition, ThemeFile } from "@/domain";

export const THEME_DRAFT_ID = "__soloist-theme-draft__";

export class ThemeImportConflictError extends Error {
  constructor(
    readonly existing: ThemeDefinition,
    readonly incoming: ThemeFile,
  ) {
    super(`A theme with id ${JSON.stringify(incoming.id)} already exists`);
    this.name = "ThemeImportConflictError";
  }
}

/**
 * The message a failed theme command should show. A Tauri command rejects with the bare string its
 * `Result<_, String>` carried, not an `Error`, so the core's precise reason — which field or rule the
 * file violated — is only reachable by reading the rejection itself.
 */
export function themeErrorMessage(cause: unknown, fallback: string): string {
  if (cause instanceof Error) return cause.message;
  if (typeof cause === "string" && cause.trim().length > 0) return cause;
  return fallback;
}

export function serializeTheme(theme: ThemeFile): string {
  return `${JSON.stringify(theme, null, 2)}\n`;
}

export function serializeNewTheme(theme: ThemeFile): string {
  const draft: Partial<ThemeFile> = { ...theme };
  delete draft.id;
  return `${JSON.stringify(draft, null, 2)}\n`;
}
