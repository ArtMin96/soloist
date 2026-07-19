// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  onDomainEvent,
  setDefaultTemplate,
  templateCreate,
  templateDefaults,
  templateDelete,
  templateRead,
  templates as listTemplates,
} from "@/api";
import type { DomainEvent, TemplateKind, TemplateSummary, TemplateView } from "@/domain";
import { useTemplates } from "@/store/useTemplates";

vi.mock("@/api", () => ({
  templates: vi.fn(),
  templateDefaults: vi.fn(),
  setDefaultTemplate: vi.fn(),
  templateCreate: vi.fn(),
  templateDelete: vi.fn(),
  templateRead: vi.fn(),
  onDomainEvent: vi.fn(),
}));

const list = vi.mocked(listTemplates);
const defaults = vi.mocked(templateDefaults);
const setDefault = vi.mocked(setDefaultTemplate);
const create = vi.mocked(templateCreate);
const remove = vi.mocked(templateDelete);
const read = vi.mocked(templateRead);
const subscribe = vi.mocked(onDomainEvent);

// Two distinct projects, so "the open project's library" and "some other project's" are never the
// same thing by accident.
const OPEN_PROJECT = 7;
const OTHER_PROJECT = 8;

function summary(id: number, name: string, kind: TemplateKind): TemplateSummary {
  return { id, kind, name, description: null, placeholders: [], scope: "global", revision: 1 };
}

// Captures the domain-event handler so a test can fire `TemplateChanged`.
let handler: ((event: DomainEvent) => void) | undefined;

// Stubs the backend as two separate libraries keyed by scope, the way the core stores them: the
// global one and the open project's. Anything asked for under another project id resolves empty.
// Called again mid-test to stand for a write that landed underneath the panel.
function mockLibraries(
  globals: TemplateSummary[] = [summary(1, "daily", "scratchpad")],
  projectOwned: TemplateSummary[] = [],
) {
  list.mockImplementation((kind, project) => {
    if (kind !== "scratchpad") return Promise.resolve([]);
    if (project == null) return Promise.resolve(globals);
    return Promise.resolve(project === OPEN_PROJECT ? projectOwned : []);
  });
}

function setup(
  globals: TemplateSummary[] = [summary(1, "daily", "scratchpad")],
  projectOwned: TemplateSummary[] = [],
) {
  handler = undefined;
  mockLibraries(globals, projectOwned);
  defaults.mockResolvedValue({ scratchpad: 1, todo: null });
  subscribe.mockImplementation((fn) => {
    handler = fn;
    return Promise.resolve(() => {});
  });
}

afterEach(() => vi.clearAllMocks());

describe("useTemplates", () => {
  it("loads every kind and the defaults once on mount", async () => {
    setup();
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));

    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));
    expect(result.current.lists.prompt.global).toHaveLength(0);
    expect(result.current.lists.todo.global).toHaveLength(0);
    expect(result.current.defaults).toEqual({ scratchpad: 1, todo: null });
  });

  it("separates the global library from the open project's", async () => {
    setup([summary(1, "daily", "scratchpad")], [summary(2, "sprint", "scratchpad")]);
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));

    await waitFor(() => expect(result.current.lists.scratchpad.project).toHaveLength(1));
    expect(result.current.lists.scratchpad.global.map((t) => t.name)).toEqual(["daily"]);
    expect(result.current.lists.scratchpad.project.map((t) => t.name)).toEqual(["sprint"]);
  });

  it("re-reads the changed kind and the defaults on a global TemplateChanged", async () => {
    setup();
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));

    // A delete cleared the default in core; the event drives a re-read of both the kind and defaults.
    mockLibraries([], []);
    defaults.mockResolvedValue({ scratchpad: null, todo: null });
    act(() => handler?.({ type: "TemplateChanged", kind: "scratchpad", project: null }));

    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(0));
    await waitFor(() => expect(result.current.defaults.scratchpad).toBeNull());
  });

  // The event's scope decides which list is re-read. Without it a project-scoped write (the MCP
  // default) sends the panel back to the global library, which did not change — the template an
  // agent just authored never appears.
  it("re-reads the project list, not the global one, on a project-scoped TemplateChanged", async () => {
    setup([summary(1, "daily", "scratchpad")], []);
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));

    // An agent wrote into the open project's library; the global one is untouched.
    mockLibraries([summary(1, "daily", "scratchpad")], [summary(2, "sprint", "scratchpad")]);
    act(() => handler?.({ type: "TemplateChanged", kind: "scratchpad", project: OPEN_PROJECT }));

    await waitFor(() =>
      expect(result.current.lists.scratchpad.project.map((t) => t.name)).toEqual(["sprint"]),
    );
    expect(result.current.lists.scratchpad.global.map((t) => t.name)).toEqual(["daily"]);
  });

  it("ignores a change in a project it is not showing", async () => {
    setup([summary(1, "daily", "scratchpad")], [summary(2, "sprint", "scratchpad")]);
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.project).toHaveLength(1));

    // Were this event's project ignored, the re-read would empty the open project's list — showing
    // the state of a library the panel is not looking at.
    mockLibraries([summary(1, "daily", "scratchpad"), summary(3, "weekly", "scratchpad")], []);
    act(() => handler?.({ type: "TemplateChanged", kind: "scratchpad", project: OTHER_PROJECT }));
    // A global event the panel *does* act on, as a barrier: once its re-read has landed, the
    // ignored one has had every chance to land too.
    act(() => handler?.({ type: "TemplateChanged", kind: "scratchpad", project: null }));

    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(2));
    expect(result.current.lists.scratchpad.project.map((t) => t.name)).toEqual(["sprint"]);
  });

  it("holds an empty project list while no project is open", async () => {
    setup([summary(1, "daily", "scratchpad")], [summary(2, "sprint", "scratchpad")]);
    const { result } = renderHook(() => useTemplates(null));

    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));
    expect(result.current.lists.scratchpad.project).toHaveLength(0);
  });

  it("shows a selected default at once, then settles on what the core stored", async () => {
    setup();
    // The core clamps the selection: `prompt` has no seed default, so the write echoes back a
    // record the optimistic value guessed wrong about.
    setDefault.mockResolvedValue({ scratchpad: 2, todo: null });
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.defaults.scratchpad).toBe(1));

    act(() => result.current.setDefault("scratchpad", 3));
    // Shown immediately, before the write resolves — the radio must not lag a round trip behind.
    expect(result.current.defaults.scratchpad).toBe(3);

    // Then the core's answer wins, so the panel never keeps showing a selection that was not stored.
    await waitFor(() => expect(result.current.defaults.scratchpad).toBe(2));
  });

  it("falls back to the stored defaults when the selection fails to persist", async () => {
    setup();
    setDefault.mockRejectedValue("settings are read-only");
    defaults.mockResolvedValue({ scratchpad: 1, todo: null });
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.defaults.scratchpad).toBe(1));

    act(() => result.current.setDefault("scratchpad", 3));
    expect(result.current.defaults.scratchpad).toBe(3);

    // The write was refused, so the optimistic value is fiction. Re-reading disk truth is what stops
    // the panel from showing a default that seeding will never use.
    await waitFor(() => expect(result.current.defaults.scratchpad).toBe(1));
  });

  it("duplicates a template under a free copy name from the source's content", async () => {
    setup([summary(1, "daily", "scratchpad"), summary(2, "daily copy", "scratchpad")]);
    const source: TemplateView = {
      id: 1,
      kind: "scratchpad",
      name: "daily",
      description: "notes",
      body: "## Plan",
      placeholders: [],
      scope: "global",
      revision: 3,
    };
    read.mockResolvedValue(source);
    create.mockResolvedValue({ ...source, id: 9, name: "daily copy 2" });
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(2));

    await act(async () => {
      await result.current.duplicate("scratchpad", "global", "daily");
    });
    // "daily copy" is taken, so the copy takes the next free slot, carrying the source's content.
    expect(create).toHaveBeenCalledWith("scratchpad", null, "daily copy 2", "notes", "## Plan");
  });

  // A copy lands beside its source, so a name free in the project library is free even when the
  // global library already holds it.
  it("decides a copy's name against its own scope's names alone", async () => {
    setup([summary(1, "daily copy", "scratchpad")], [summary(2, "daily", "scratchpad")]);
    const source: TemplateView = {
      id: 2,
      kind: "scratchpad",
      name: "daily",
      description: null,
      body: "## Plan",
      placeholders: [],
      scope: "project",
      revision: 1,
    };
    read.mockResolvedValue(source);
    create.mockResolvedValue({ ...source, id: 9, name: "daily copy" });
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.project).toHaveLength(1));

    await act(async () => {
      await result.current.duplicate("scratchpad", "project", "daily");
    });
    expect(create).toHaveBeenCalledWith("scratchpad", OPEN_PROJECT, "daily copy", null, "## Plan");
  });

  it("reports whether a delete removed anything", async () => {
    setup();
    remove.mockResolvedValue(true);
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));

    await expect(result.current.remove("scratchpad", "global", "daily")).resolves.toBeUndefined();
  });

  // The panel decides what a refused write looks like on screen (and, for a delete, keeps the editor
  // open), so both write actions must hand the reason back rather than absorb it into the read state.
  it("rejects a refused delete instead of absorbing it into the load error", async () => {
    setup();
    remove.mockRejectedValue("template is locked");
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));

    await expect(result.current.remove("scratchpad", "global", "daily")).rejects.toBe(
      "template is locked",
    );
    expect(result.current.error).toBeNull();
  });

  it("rejects a refused duplicate instead of absorbing it into the load error", async () => {
    setup();
    read.mockResolvedValue({
      id: 1,
      kind: "scratchpad",
      name: "daily",
      description: null,
      body: "## Plan",
      placeholders: [],
      scope: "global",
      revision: 1,
    });
    create.mockRejectedValue("name is longer than 200 characters");
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));

    await expect(result.current.duplicate("scratchpad", "global", "daily")).rejects.toBe(
      "name is longer than 200 characters",
    );
    expect(result.current.error).toBeNull();
  });

  it("creates with a null description when the field is blank", async () => {
    setup();
    create.mockResolvedValue({
      id: 9,
      kind: "todo",
      name: "chore",
      description: null,
      body: "b",
      placeholders: [],
      scope: "global",
      revision: 1,
    });
    const { result } = renderHook(() => useTemplates(OPEN_PROJECT));
    await waitFor(() => expect(result.current.lists.scratchpad.global).toHaveLength(1));

    await act(async () => {
      await result.current.create("todo", "global", "chore", "   ", "b");
    });
    expect(create).toHaveBeenCalledWith("todo", null, "chore", null, "b");
  });
});
