// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { renderDiagram } from "./engine";
import { useDiagramRender } from "./useDiagramRender";

// Only the engine is mocked, so the hook's own sequencing is what these assertions exercise.
vi.mock("./engine", () => ({ renderDiagram: vi.fn() }));

const engine = vi.mocked(renderDiagram);

afterEach(() => vi.clearAllMocks());

const svg = (source: string) => `<svg>${source}</svg>`;

describe("useDiagramRender", () => {
  it("never draws a source that was superseded before it could start", async () => {
    // A Mermaid render costs about half a second while an edit can land every debounce interval, so a
    // surface that queued every source it passed through would keep drawing long after typing stopped
    // — and every one of those draws but the last produces output nothing will ever show.
    const drawn: string[] = [];
    let releaseFirst = () => {};
    engine.mockImplementation((source: string) => {
      drawn.push(source);
      if (source === "first") {
        return new Promise((resolve) => {
          releaseFirst = () => resolve({ svg: svg("first") });
        });
      }
      return Promise.resolve({ svg: svg(source) });
    });

    const { result, rerender } = renderHook(({ source }) => useDiagramRender(source), {
      initialProps: { source: "first" },
    });
    await waitFor(() => expect(drawn).toEqual(["first"]));

    rerender({ source: "passed through" });
    rerender({ source: "latest" });
    await act(async () => releaseFirst());

    await waitFor(() => expect(result.current.drawn).toBe(svg("latest")));
    expect(drawn).toEqual(["first", "latest"]);
  });

  it("does not report a failure for a source that has already been replaced", async () => {
    // The broken source is gone from the editor by the time its failure comes back. Surfacing it would
    // put an error on screen for text the editor no longer holds, and mark a valid diagram invalid.
    let releaseBroken = () => {};
    let started = false;
    engine.mockImplementation((source: string) => {
      if (source === "broken") {
        started = true;
        return new Promise((resolve) => {
          releaseBroken = () => resolve({ error: "Parse error on line 2" });
        });
      }
      return Promise.resolve({ svg: svg(source) });
    });
    const onParse = vi.fn();

    const { result, rerender } = renderHook(({ source }) => useDiagramRender(source, onParse), {
      initialProps: { source: "broken" },
    });
    await waitFor(() => expect(started).toBe(true));

    rerender({ source: "fixed" });
    await act(async () => releaseBroken());

    await waitFor(() => expect(result.current.drawn).toBe(svg("fixed")));
    expect(result.current.status).toBe("drawn");
    expect(onParse).not.toHaveBeenCalledWith(false);
  });

  it("keeps the last diagram that drew when the current source fails", async () => {
    engine.mockResolvedValue({ svg: svg("good") });
    const { result, rerender } = renderHook(({ source }) => useDiagramRender(source), {
      initialProps: { source: "good" },
    });
    await waitFor(() => expect(result.current.status).toBe("drawn"));

    engine.mockResolvedValue({ error: "Parse error on line 2" });
    rerender({ source: "broken" });

    await waitFor(() => expect(result.current.status).toBe("error"));
    expect(result.current.drawn).toBe(svg("good"));
  });
});
