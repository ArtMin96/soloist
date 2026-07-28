import { describe, expect, it } from "vitest";
import { quoteShellPath, quoteShellPaths } from "@/lib/shellQuote";

// Each expectation below is the exact word a POSIX shell has to be handed to read the path back as
// the bytes it started as. Inside single quotes a shell performs no expansion and honours no escape
// character, so every one of these paths comes back whole — except an apostrophe, which cannot
// appear in a quoted run at all and is spelled by closing the run, escaping it, and reopening.

describe("quoteShellPath", () => {
  it("keeps a path with a space one word", () => {
    expect(quoteShellPath("/home/dell/My Screenshots/a.png")).toBe(
      "'/home/dell/My Screenshots/a.png'",
    );
  });

  it("closes and reopens the quoting around a single quote", () => {
    expect(quoteShellPath("/home/dell/O'Brien/notes.md")).toBe("'/home/dell/O'\\''Brien/notes.md'");
  });

  it("carries several single quotes, each spelled out", () => {
    expect(quoteShellPath("'a'b'")).toBe("''\\''a'\\''b'\\'''");
  });

  it("passes double quotes through, since single quoting already makes them literal", () => {
    expect(quoteShellPath('/tmp/say "hello".txt')).toBe("'/tmp/say \"hello\".txt'");
  });

  // A single-quoted run has no escape character at all, so a backslash is not consumed as one and
  // must not be doubled on the way in — a path holding one comes back with exactly the one it had.
  it("passes a backslash through, since single quotes process no escapes", () => {
    expect(quoteShellPath("/tmp/a\\b/c.png")).toBe("'/tmp/a\\b/c.png'");
  });

  it("passes a newline through, so a path that spans lines stays one word", () => {
    expect(quoteShellPath("/tmp/two\nlines.txt")).toBe("'/tmp/two\nlines.txt'");
  });

  // The characters a shell would otherwise act on rather than pass through: a variable, a command
  // substitution, a glob, a separator, a redirect. Single quoting makes every one of them inert.
  it("leaves substitutions, globs and command syntax inert", () => {
    expect(quoteShellPath("/tmp/$HOME `id` $(id) *.png > out;rm")).toBe(
      "'/tmp/$HOME `id` $(id) *.png > out;rm'",
    );
  });

  it("quotes an empty path rather than emitting nothing", () => {
    expect(quoteShellPath("")).toBe("''");
  });
});

describe("quoteShellPaths", () => {
  it("separates the paths with a single space", () => {
    expect(quoteShellPaths(["/a", "/b"])).toBe("'/a' '/b'");
  });

  it("quotes each path of a multi-file drop independently", () => {
    expect(
      quoteShellPaths(["/tmp/a.png", "/home/dell/My Screenshots/b.png", "/tmp/O'Brien c.txt"]),
    ).toBe("'/tmp/a.png' '/home/dell/My Screenshots/b.png' '/tmp/O'\\''Brien c.txt'");
  });

  it("emits nothing for no paths", () => {
    expect(quoteShellPaths([])).toBe("");
  });
});
