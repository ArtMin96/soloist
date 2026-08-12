import { describe, expect, it } from "vitest";
import { themeErrorMessage } from "@/theme/io";

const FALLBACK = "The theme could not be imported.";

describe("themeErrorMessage", () => {
  it("surfaces the bare string a rejected Tauri command carries", () => {
    expect(
      themeErrorMessage(
        "invalid theme file: unknown field `tags`, expected one of `version`, `id`, `name`",
        FALLBACK,
      ),
    ).toBe("invalid theme file: unknown field `tags`, expected one of `version`, `id`, `name`");
  });

  it("surfaces the message of a thrown Error", () => {
    expect(themeErrorMessage(new Error("The imported theme was not returned"), FALLBACK)).toBe(
      "The imported theme was not returned",
    );
  });

  it("falls back when the rejection carries no readable message", () => {
    expect(themeErrorMessage(undefined, FALLBACK)).toBe(FALLBACK);
    expect(themeErrorMessage("   ", FALLBACK)).toBe(FALLBACK);
    expect(themeErrorMessage({ code: 500 }, FALLBACK)).toBe(FALLBACK);
  });
});
