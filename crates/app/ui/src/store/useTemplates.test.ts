// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  onDomainEvent,
  setDefaultTemplate,
  templateCreate,
  templateDefaults,
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
const read = vi.mocked(templateRead);
const subscribe = vi.mocked(onDomainEvent);

function summary(id: number, name: string, kind: TemplateKind): TemplateSummary {
  return { id, kind, name, description: null, placeholders: [], scope: "global", revision: 1 };
}

// Captures the domain-event handler so a test can fire `TemplateChanged`.
let handler: ((event: DomainEvent) => void) | undefined;

function setup(scratchpads: TemplateSummary[] = [summary(1, "daily", "scratchpad")]) {
  handler = undefined;
  list.mockImplementation((kind) => Promise.resolve(kind === "scratchpad" ? scratchpads : []));
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
    const { result } = renderHook(() => useTemplates());

    await waitFor(() => expect(result.current.lists.scratchpad).toHaveLength(1));
    expect(list).toHaveBeenCalledWith("prompt");
    expect(list).toHaveBeenCalledWith("scratchpad");
    expect(list).toHaveBeenCalledWith("todo");
    expect(result.current.defaults).toEqual({ scratchpad: 1, todo: null });
  });

  it("re-reads the changed kind and the defaults on TemplateChanged", async () => {
    setup();
    const { result } = renderHook(() => useTemplates());
    await waitFor(() => expect(result.current.lists.scratchpad).toHaveLength(1));

    // A delete cleared the default in core; the event drives a re-read of both the kind and defaults.
    list.mockImplementation(() => Promise.resolve([]));
    defaults.mockResolvedValue({ scratchpad: null, todo: null });
    act(() => handler?.({ type: "TemplateChanged", kind: "scratchpad" }));

    await waitFor(() => expect(result.current.lists.scratchpad).toHaveLength(0));
    await waitFor(() => expect(result.current.defaults.scratchpad).toBeNull());
  });

  it("selects a default optimistically and persists it", async () => {
    setup();
    setDefault.mockResolvedValue({ scratchpad: 1, todo: null });
    const { result } = renderHook(() => useTemplates());
    await waitFor(() => expect(result.current.lists.scratchpad).toHaveLength(1));

    act(() => result.current.setDefault("scratchpad", 1));
    expect(result.current.defaults.scratchpad).toBe(1);
    expect(setDefault).toHaveBeenCalledWith("scratchpad", 1);
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
    const { result } = renderHook(() => useTemplates());
    await waitFor(() => expect(result.current.lists.scratchpad).toHaveLength(2));

    await act(async () => {
      await result.current.duplicate("scratchpad", "daily");
    });
    // "daily copy" is taken, so the copy takes the next free slot, carrying the source's content.
    expect(create).toHaveBeenCalledWith("scratchpad", "daily copy 2", "notes", "## Plan");
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
    const { result } = renderHook(() => useTemplates());
    await waitFor(() => expect(result.current.lists.scratchpad).toHaveLength(1));

    await act(async () => {
      await result.current.create("todo", "chore", "   ", "b");
    });
    expect(create).toHaveBeenCalledWith("todo", "chore", null, "b");
  });
});
