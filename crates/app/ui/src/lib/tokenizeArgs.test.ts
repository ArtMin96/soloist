import { describe, expect, it } from "vitest";
import { tokenizeArgs } from "@/lib/tokenizeArgs";

describe("tokenizeArgs", () => {
  it("splits bare flags on whitespace", () => {
    expect(tokenizeArgs("--model sonnet")).toEqual(["--model", "sonnet"]);
  });

  it("keeps a double-quoted value as one token, without the quotes", () => {
    expect(tokenizeArgs('--append-system-prompt "be concise"')).toEqual([
      "--append-system-prompt",
      "be concise",
    ]);
  });

  it("keeps a single-quoted value as one token", () => {
    expect(tokenizeArgs("--dir 'my project'")).toEqual(["--dir", "my project"]);
  });

  it("ignores surrounding and repeated whitespace", () => {
    expect(tokenizeArgs("  --resume   --verbose  ")).toEqual(["--resume", "--verbose"]);
  });

  it("returns no tokens for empty or whitespace-only input", () => {
    expect(tokenizeArgs("")).toEqual([]);
    expect(tokenizeArgs("   ")).toEqual([]);
  });
});
