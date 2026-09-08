// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

// The read, the event subscription, and the resync signal are the IPC boundary; mock them so the
// test drives the hook's own logic — seeding, event routing, and frame coalescing.
vi.mock("@/api", () => ({
  projectWork: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
  onResync: vi.fn(() => Promise.resolve(() => {})),
}));

import { onDomainEvent, projectWork } from "@/api";
import type { ProcessView, ProjectView, ProjectWork } from "@/domain";
import { useProjectWork } from "@/store/useProjectWork";

const read = vi.mocked(projectWork);
const domainEvent = vi.mocked(onDomainEvent);

afterEach(() => vi.clearAllMocks());

function project(id: number): ProjectView {
  return { id, name: `project-${id}`, root: `/tmp/project-${id}`, icon: null };
}

function processView(id: number, ownerProject: number): ProcessView {
  return {
    id,
    project: ownerProject,
    kind: "Agent",
    label: `agent-${id}`,
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
  };
}

function empty(id: number): ProjectWork {
  return { project: id, todos: [], scratchpads: [] };
}

function touched(id: number): ProjectWork {
  return {
    project: id,
    todos: [
      {
        id: 1,
        title: "touched",
        status: "open",
        participants: [{ process: 9, label: "agent-9", role: "reading" }],
      },
    ],
    scratchpads: [],
  };
}

/** The hook's own domain-event subscriber, as the Tauri bridge would call it. */
function emit(event: Parameters<Parameters<typeof onDomainEvent>[0]>[0]) {
  const handler = domainEvent.mock.calls[0]?.[0];
  if (!handler) throw new Error("no domain-event subscriber registered");
  act(() => handler(event));
}

async function frame() {
  await act(async () => {
    await new Promise((resolve) => requestAnimationFrame(resolve));
  });
}

describe("useProjectWork", () => {
  it("seeds a map entry for every project on mount", async () => {
    read.mockImplementation((id: number) => Promise.resolve(empty(id)));
    const { result } = renderHook(() => useProjectWork([project(1), project(2)], []));
    await waitFor(() => expect(result.current.size).toBe(2));
    expect(result.current.get(1)).toEqual(empty(1));
    expect(result.current.get(2)).toEqual(empty(2));
  });

  it("a SessionWorkChanged for a process the store knows re-reads only that project", async () => {
    read.mockImplementation((id: number) => Promise.resolve(empty(id)));
    const { result } = renderHook(() =>
      useProjectWork([project(1), project(2)], [processView(9, 1)]),
    );
    await waitFor(() => expect(result.current.size).toBe(2));
    const beforeB = result.current.get(2);

    read.mockImplementation((id: number) => Promise.resolve(touched(id)));
    emit({ type: "SessionWorkChanged", process: 9 });
    await frame();

    await waitFor(() => expect(result.current.get(1)?.todos.length).toBe(1));
    expect(result.current.get(2)).toBe(beforeB);
  });

  it("a SessionWorkChanged for an unknown process re-reads every project", async () => {
    read.mockImplementation((id: number) => Promise.resolve(empty(id)));
    const { result } = renderHook(() => useProjectWork([project(1), project(2)], []));
    await waitFor(() => expect(result.current.size).toBe(2));

    read.mockImplementation((id: number) => Promise.resolve(touched(id)));
    emit({ type: "SessionWorkChanged", process: 999 });
    await frame();

    await waitFor(() => expect(result.current.get(1)?.todos.length).toBe(1));
    await waitFor(() => expect(result.current.get(2)?.todos.length).toBe(1));
  });

  it("a TodoChanged for project B leaves project A's entry object identical", async () => {
    read.mockImplementation((id: number) => Promise.resolve(empty(id)));
    const { result } = renderHook(() => useProjectWork([project(1), project(2)], []));
    await waitFor(() => expect(result.current.size).toBe(2));
    const beforeA = result.current.get(1);

    read.mockImplementation((id: number) => Promise.resolve(touched(id)));
    emit({ type: "TodoChanged", project: 2, id: 1 });
    await frame();

    await waitFor(() => expect(result.current.get(2)?.todos.length).toBe(1));
    expect(result.current.get(1)).toBe(beforeA);
  });

  it("a burst of events in one frame coalesces to a single read per project", async () => {
    read.mockImplementation((id: number) => Promise.resolve(empty(id)));
    const { result } = renderHook(() => useProjectWork([project(1), project(2)], []));
    await waitFor(() => expect(result.current.size).toBe(2));
    const seeded = read.mock.calls.length;

    read.mockImplementation((id: number) => Promise.resolve(touched(id)));
    emit({ type: "TodoChanged", project: 1, id: 1 });
    emit({ type: "TodoChanged", project: 1, id: 2 });
    emit({ type: "TodoChanged", project: 1, id: 3 });

    await waitFor(() => expect(result.current.get(1)?.todos.length).toBe(1));
    expect(
      read.mock.calls.length - seeded,
      "a chatty run costs one re-read per project per frame, not one per event",
    ).toBe(1);
  });

  it("a project removed from `projects` loses its entry", async () => {
    read.mockImplementation((id: number) => Promise.resolve(empty(id)));
    const { result, rerender } = renderHook(({ projects }) => useProjectWork(projects, []), {
      initialProps: { projects: [project(1), project(2)] } as { projects: ProjectView[] },
    });
    await waitFor(() => expect(result.current.size).toBe(2));

    rerender({ projects: [project(1)] });
    await waitFor(() => expect(result.current.has(2)).toBe(false));
    expect(result.current.has(1)).toBe(true);
  });
});
