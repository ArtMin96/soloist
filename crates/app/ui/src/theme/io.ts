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

export function serializeTheme(theme: ThemeFile): string {
  return `${JSON.stringify(theme, null, 2)}\n`;
}

export function serializeNewTheme(theme: ThemeFile): string {
  const draft: Partial<ThemeFile> = { ...theme };
  delete draft.id;
  return `${JSON.stringify(draft, null, 2)}\n`;
}
