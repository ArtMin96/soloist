// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useMermaidTheme } from "@/lib/mermaid/useMermaidTheme";

afterEach(() => {
  document.documentElement.className = "";
  delete document.documentElement.dataset.themeSignature;
});

describe("useMermaidTheme", () => {
  it("tracks the root's palette signature as its class and dataset attribute change", async () => {
    const { result } = renderHook(() => useMermaidTheme());
    expect(result.current).toBe("legacy:light");

    act(() => document.documentElement.classList.add("dark"));
    await waitFor(() => expect(result.current).toBe("legacy:dark"));

    act(() => {
      document.documentElement.dataset.themeSignature = "dracula";
    });
    await waitFor(() => expect(result.current).toBe("dracula:dark"));
  });

  it("reads whatever the root already carries at mount, not a stale initial snapshot", () => {
    document.documentElement.classList.add("dark");
    document.documentElement.dataset.themeSignature = "poimandres";

    const { result } = renderHook(() => useMermaidTheme());

    expect(result.current).toBe("poimandres:dark");
  });
});
