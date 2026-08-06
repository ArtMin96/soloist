import { describe, expect, it } from "vitest";
import { languageOf } from "@/lib/diff/language";

describe("languageOf", () => {
  it("reads an extension that is not the language's own name", () => {
    expect(languageOf("crates/app/ui/src/App.tsx")).toBe("tsx");
    expect(languageOf("src/main.rs")).toBe("rust");
    expect(languageOf("scripts/check.sh")).toBe("shellscript");
    expect(languageOf("README.md")).toBe("markdown");
  });

  it("reads an extension that already names its language", () => {
    expect(languageOf("Cargo.toml")).toBe("toml");
    expect(languageOf("package.json")).toBe("json");
  });

  it("reads a file whose whole name says what it is", () => {
    expect(languageOf("Dockerfile")).toBe("docker");
    expect(languageOf("deploy/Makefile")).toBe("make");
  });

  it("names no language for a file there is no grammar for", () => {
    expect(languageOf("assets/icon.png")).toBeNull();
    expect(languageOf("LICENSE")).toBeNull();
    expect(languageOf(".gitignore")).toBeNull();
  });
});
