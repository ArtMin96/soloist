import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defaultAppliedTheme } from "@/theme/runtime";

const createHighlighterCore = vi.fn();
const createJavaScriptRegexEngine = vi.fn(() => ({}));
const grammarOf = vi.fn();

vi.mock("shiki/core", () => ({ createHighlighterCore }));
vi.mock("shiki/engine/javascript", () => ({ createJavaScriptRegexEngine }));
// `STARTING_LANGUAGES` empty keeps engine start and grammar loading independent of each other:
// nothing here fetches a grammar unless a test asks `ensureLanguage`/`ensureHighlighting` to.
vi.mock("@/lib/diff/language", () => ({ STARTING_LANGUAGES: [], grammarOf }));

/** A minimal stand-in for a Shiki `HighlighterCore`, tracking only what these tests read back. */
function fakeCore() {
  const languages = new Set<string>();
  return {
    getLoadedLanguages: () => Array.from(languages),
    loadLanguage: async (registrations: { name: string }[]) => {
      for (const registration of registrations) languages.add(registration.name);
    },
    loadTheme: async () => {},
    loadThemeSync: () => {},
    codeToHtml: (code: string, options: { lang: string }) =>
      `<span data-lang="${options.lang}">${code}</span>`,
    codeToHast: () => ({ type: "root" }),
  };
}

/** Let every pending continuation run, so a fire-and-forget call has settled. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

async function highlighter() {
  return import("./highlighter");
}

beforeEach(() => {
  vi.resetModules();
  createHighlighterCore.mockReset();
  createJavaScriptRegexEngine.mockReset().mockReturnValue({});
  grammarOf.mockReset();
});

afterEach(() => vi.restoreAllMocks());

describe("ensureLanguage", () => {
  it("resolves false when a grammar fails to load, and retries rather than staying broken", async () => {
    createHighlighterCore.mockResolvedValue(fakeCore());
    grammarOf.mockRejectedValueOnce(new Error("chunk fetch failed"));
    grammarOf.mockResolvedValueOnce({ default: [{ name: "python" }] });

    const { ensureLanguage } = await highlighter();

    await expect(ensureLanguage("python")).resolves.toBe(false);
    await expect(ensureLanguage("python")).resolves.toBe(true);
  });
});

describe("ensureHighlighting", () => {
  it("answers false, false and null across every reader when the engine fails to start, then recovers on retry", async () => {
    createHighlighterCore.mockRejectedValueOnce(new Error("wasm init failed"));
    const { ensureHighlighting, HIGHLIGHTER, highlightedHtml } = await highlighter();
    const theme = defaultAppliedTheme(false);

    await expect(ensureHighlighting("python", theme)).resolves.toBe(false);
    expect(HIGHLIGHTER.hasRegisteredCurrentLang("python")).toBe(false);
    expect(highlightedHtml("print(1)", "python", theme)).toBeNull();

    createHighlighterCore.mockResolvedValue(fakeCore());
    grammarOf.mockResolvedValue({ default: [{ name: "python" }] });

    await expect(ensureHighlighting("python", theme)).resolves.toBe(true);
    expect(HIGHLIGHTER.hasRegisteredCurrentLang("python")).toBe(true);
  });

  it("colours code once its language and the app theme are both loaded", async () => {
    createHighlighterCore.mockResolvedValue(fakeCore());
    grammarOf.mockResolvedValue({ default: [{ name: "rust" }] });
    const theme = defaultAppliedTheme(true);

    const { ensureHighlighting, highlightedHtml } = await highlighter();
    await expect(ensureHighlighting("rust", theme)).resolves.toBe(true);

    expect(highlightedHtml("fn main() {}", "rust", theme)).toBe(
      '<span data-lang="rust">fn main() {}</span>',
    );
  });

  it("never leaves an unhandled rejection behind for a caller that only attaches `.then`", async () => {
    // Mirrors the actual call sites (`DiffViewer.tsx`, `FilePreview.tsx`): fire-and-forget, no
    // `.catch`. A promise this rejects is an unhandled rejection at both of them.
    createHighlighterCore.mockRejectedValueOnce(new Error("wasm init failed"));
    const { ensureHighlighting } = await highlighter();
    const theme = defaultAppliedTheme(false);

    const rejections: unknown[] = [];
    const onUnhandledRejection = (reason: unknown) => rejections.push(reason);
    process.on("unhandledRejection", onUnhandledRejection);

    void ensureHighlighting("rust", theme).then(() => {});
    await flush();

    process.off("unhandledRejection", onUnhandledRejection);
    expect(rejections).toEqual([]);
  });
});
