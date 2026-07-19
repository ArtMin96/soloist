// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { templateCreate, templateRead } from "@/api";
import type { TemplateKind, TemplateSummary } from "@/domain";
import { TemplatesPanel } from "@/components/settings/TemplatesPanel";

vi.mock("@/api", () => ({
  templates: vi.fn(),
  templateDefaults: vi.fn(),
  setDefaultTemplate: vi.fn(),
  templateCreate: vi.fn(),
  templateDelete: vi.fn(),
  templateRead: vi.fn(),
  templateUpdate: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

// Stub the lazy rich editor so the panel test never mounts TipTap — the editor is covered on its own.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { ariaLabel?: string; onChange: (v: string) => void }) => (
    <textarea
      aria-label={props.ariaLabel}
      onChange={(event) => props.onChange(event.target.value)}
    />
  ),
}));

import { templates as listTemplates, templateDefaults } from "@/api";

const list = vi.mocked(listTemplates);
const defaults = vi.mocked(templateDefaults);
const read = vi.mocked(templateRead);
const create = vi.mocked(templateCreate);

function summary(id: number, name: string, kind: TemplateKind): TemplateSummary {
  return { id, kind, name, description: null, placeholders: [], scope: "global", revision: 1 };
}

function seed(scratchpads: TemplateSummary[] = [summary(1, "daily", "scratchpad")]) {
  list.mockImplementation((kind) => Promise.resolve(kind === "scratchpad" ? scratchpads : []));
  defaults.mockResolvedValue({ scratchpad: 1, todo: null });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("TemplatesPanel", () => {
  it("groups templates by kind and shows a default selector only for seedable kinds with templates", async () => {
    seed();
    render(<TemplatesPanel />);

    // The row is a real button (the Radix default selector is a combobox, excluded by role).
    await screen.findByRole("button", { name: "Duplicate daily" });
    // All three kinds are sectioned (the Prompt section delivers the reserved prompt-templates view).
    expect(screen.getByRole("heading", { name: "Prompt" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Scratchpad" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Todo" })).toBeTruthy();
    // The default selector appears for the scratchpad kind (it has a template), not for the empty
    // todo kind and never for prompts.
    expect(screen.getAllByText("Default template")).toHaveLength(1);
  });

  it("duplicates a template from its stored content", async () => {
    seed();
    read.mockResolvedValue({
      id: 1,
      kind: "scratchpad",
      name: "daily",
      description: "notes",
      body: "## Plan",
      placeholders: [],
      scope: "global",
      revision: 2,
    });
    create.mockResolvedValue({
      id: 9,
      kind: "scratchpad",
      name: "daily copy",
      description: "notes",
      body: "## Plan",
      placeholders: [],
      scope: "global",
      revision: 1,
    });
    render(<TemplatesPanel />);
    await screen.findByRole("button", { name: "Duplicate daily" });

    fireEvent.click(screen.getByRole("button", { name: "Duplicate daily" }));

    await waitFor(() => expect(read).toHaveBeenCalledWith("scratchpad", "daily"));
    await waitFor(() =>
      expect(create).toHaveBeenCalledWith("scratchpad", "daily copy", "notes", "## Plan"),
    );
  });

  it("drills into a create form and posts a new template", async () => {
    seed([]);
    create.mockResolvedValue({
      id: 9,
      kind: "prompt",
      name: "review",
      description: null,
      body: "Review {{pr}}",
      placeholders: ["pr"],
      scope: "global",
      revision: 1,
    });
    render(<TemplatesPanel />);
    await waitFor(() => expect(screen.getByRole("heading", { name: "Prompt" })).toBeTruthy());

    // Open the first section's create form (Prompt).
    fireEvent.click(screen.getAllByRole("button", { name: /New template/ })[0]);
    const createButton = await screen.findByRole("button", { name: /Create template/ });

    fireEvent.change(screen.getByLabelText("Template name"), { target: { value: "review" } });
    fireEvent.change(screen.getByLabelText("Template body"), {
      target: { value: "Review {{pr}}" },
    });
    fireEvent.click(createButton);

    await waitFor(() =>
      expect(create).toHaveBeenCalledWith("prompt", "review", null, "Review {{pr}}"),
    );
  });

  it("shows the not-found state when opening a deleted template", async () => {
    seed();
    read.mockRejectedValue("no template under that name");
    render(<TemplatesPanel />);
    await screen.findByRole("button", { name: "Duplicate daily" });

    // Opening the row (a button named for the template) reads the full template; a rejection surfaces
    // the core's message.
    fireEvent.click(screen.getByRole("button", { name: "daily" }));
    await waitFor(() => expect(screen.getByText("no template under that name")).toBeTruthy());
  });
});
