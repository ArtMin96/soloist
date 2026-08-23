// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { diagramRead, diagramRename, diagramWrite } from "@/api";
import { useDiagramEditor } from "@/store/useDiagramEditor";
import type { SaveOutcome } from "@/store/saveOutcome";
import { expectSupersededReadIsDiscarded } from "@/test/loadRaceContract";
import type { DiagramView } from "@/domain";

vi.mock("@/api", () => ({
  diagramRead: vi.fn(),
  diagramWrite: vi.fn(),
  diagramRename: vi.fn(),
}));

const view = (name: string, revision: number, source = "flowchart TD\n  A-->B"): DiagramView => ({
  id: 2,
  name,
  tags: [],
  archived: false,
  revision,
  source,
});

/** Opens `name` in a fresh editor hook, with the initial read resolved at `revision`. */
async function openedEditor(name: string, revision = 3) {
  vi.mocked(diagramRead).mockResolvedValueOnce(view(name, revision));
  const { result } = renderHook(() => useDiagramEditor(7));
  act(() => result.current.open(name));
  await waitFor(() => expect(result.current.initialSource).not.toBeNull());
  return result;
}

afterEach(() => vi.clearAllMocks());

describe("useDiagramEditor open", () => {
  it("discards a superseded read that resolves after the current diagram", async () => {
    const { result } = await expectSupersededReadIsDiscarded({
      useStore: () => useDiagramEditor(7),
      readFn: vi.mocked(diagramRead),
      open: (store, target) => store.open(target.name),
      snapshotOf: (store) => ({
        identity: store.name,
        content: store.initialSource,
        revision: store.baseRevision,
      }),
      snapshotIn: (target) => ({
        identity: target.name,
        content: target.source,
        revision: target.revision,
      }),
      first: view("auth-flow", 3, "flowchart TD\n  A-->B"),
      second: view("data-model", 8, "flowchart LR\n  D-->E"),
    });

    vi.mocked(diagramWrite).mockResolvedValueOnce(view("data-model", 9));
    await act(() => result.current.save("flowchart LR\n  D-->F"));
    expect(diagramWrite).toHaveBeenCalledWith(7, "data-model", "flowchart LR\n  D-->F", 8);
  });
});

describe("useDiagramEditor save", () => {
  it("advances the base revision on a successful write and stays clean", async () => {
    const result = await openedEditor("auth-flow", 3);
    vi.mocked(diagramWrite).mockResolvedValueOnce(view("auth-flow", 4));

    let outcome: SaveOutcome | undefined;
    await act(async () => {
      outcome = await result.current.save("flowchart TD\n  A-->C");
    });

    expect(outcome).toBe("saved");
    // The guard the write carried was the revision it was opened at.
    expect(diagramWrite).toHaveBeenCalledWith(7, "auth-flow", "flowchart TD\n  A-->C", 3);
    expect(result.current.baseRevision).toBe(4);
    expect(result.current.conflict).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("flags a conflict when a refused write reveals a moved-on revision", async () => {
    const result = await openedEditor("auth-flow", 3);
    vi.mocked(diagramWrite).mockRejectedValueOnce("stale revision");
    // The re-read after the refusal shows the document now sits at a newer revision.
    vi.mocked(diagramRead).mockResolvedValueOnce(view("auth-flow", 5));

    let outcome: SaveOutcome | undefined;
    await act(async () => {
      outcome = await result.current.save("flowchart TD\n  A-->C");
    });

    expect(outcome).toBe("refused");
    expect(result.current.conflict).toEqual({ actual: 5 });
    expect(result.current.error).toBeNull();
  });

  it("surfaces a plain error when a refused write is not a revision move", async () => {
    const result = await openedEditor("auth-flow", 3);
    vi.mocked(diagramWrite).mockRejectedValueOnce("invalid diagram");
    // The re-read shows the same revision, so the refusal was not a concurrent edit.
    vi.mocked(diagramRead).mockResolvedValueOnce(view("auth-flow", 3));

    let outcome: SaveOutcome | undefined;
    await act(async () => {
      outcome = await result.current.save("flowchart TD\n  A-->C");
    });

    expect(outcome).toBe("refused");
    expect(result.current.conflict).toBeNull();
    expect(result.current.error).toBe("invalid diagram");
  });
});

describe("useDiagramEditor rename", () => {
  it("follows the open document to its new handle without re-reading it", async () => {
    const result = await openedEditor("auth-flow", 3);
    const mountKey = result.current.mountKey;
    vi.mocked(diagramRename).mockResolvedValueOnce(view("Auth flow", 3));

    await act(() => result.current.rename("Auth flow"));

    expect(diagramRename).toHaveBeenCalledWith(7, "auth-flow", "Auth flow");
    expect(result.current.name).toBe("Auth flow");
    // A rename is not an edit: no remount, so the draft and its history survive.
    expect(result.current.mountKey).toBe(mountKey);
    expect(diagramRead).toHaveBeenCalledTimes(1);
  });

  it("rethrows a refusal and keeps the editor on the name it had", async () => {
    const result = await openedEditor("auth-flow", 3);
    vi.mocked(diagramRename).mockRejectedValueOnce("a diagram named that already exists");

    await expect(result.current.rename("data-model")).rejects.toBe(
      "a diagram named that already exists",
    );
    expect(result.current.name).toBe("auth-flow");
    expect(result.current.error).toBeNull();
  });
});
