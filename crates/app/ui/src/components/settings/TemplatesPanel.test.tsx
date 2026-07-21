// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { templateCreate, templateDelete, templateRead } from "@/api";
import type { DomainEvent, TemplateKind, TemplateSummary } from "@/domain";
import { TemplatesPanel } from "@/components/settings/TemplatesPanel";

vi.mock("@/api", () => ({
  templates: vi.fn(),
  templateDefaults: vi.fn(),
  setDefaultTemplate: vi.fn(),
  templateCreate: vi.fn(),
  templateDelete: vi.fn(),
  templateRead: vi.fn(),
  templateUpdate: vi.fn(),
  onDomainEvent: vi.fn(),
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

import { templates as listTemplates, templateDefaults, onDomainEvent } from "@/api";

const list = vi.mocked(listTemplates);
const defaults = vi.mocked(templateDefaults);
const subscribe = vi.mocked(onDomainEvent);

// Captures the panel's domain-event handler so a test can announce a change the way the core does.
let handler: ((event: DomainEvent) => void) | undefined;
const read = vi.mocked(templateRead);
const create = vi.mocked(templateCreate);
const remove = vi.mocked(templateDelete);

// Two distinct projects: the one the panel is showing, and one whose templates must never surface.
const OPEN_PROJECT = 7;
const OTHER_PROJECT = 8;

// The accessible names of the two scratchpad groups — the panel's own vocabulary, so a rename of the
// copy moves these with it.
const GLOBAL_GROUP = "Global scratchpad templates";
const PROJECT_GROUP = "Scratchpad templates in this project";

const DAILY = {
  id: 1,
  kind: "scratchpad" as const,
  name: "daily",
  description: "notes",
  body: "## Plan",
  placeholders: [],
  scope: "global" as const,
  revision: 2,
};

// Drills from the browse list into the editor for the seeded "daily" template.
async function openDaily() {
  render(<TemplatesPanel project={OPEN_PROJECT} />);
  await screen.findByRole("button", { name: "Duplicate daily" });
  fireEvent.click(screen.getByRole("button", { name: "daily" }));
  return screen.findByRole("button", { name: "Delete template" });
}

function summary(id: number, name: string, kind: TemplateKind): TemplateSummary {
  return { id, kind, name, description: null, placeholders: [], scope: "global", revision: 1 };
}

// Stubs the backend as two separate scratchpad libraries, the way the core stores them. A read for
// any project other than the open one resolves empty.
// Restubs just the libraries, standing for a write that landed underneath the panel. Kept apart
// from `seed` so a mid-test call cannot drop the captured event handler along with it.
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

function seed(
  globals: TemplateSummary[] = [summary(1, "daily", "scratchpad")],
  projectOwned: TemplateSummary[] = [],
) {
  mockLibraries(globals, projectOwned);
  defaults.mockResolvedValue({ scratchpad: 1, todo: null });
  handler = undefined;
  subscribe.mockImplementation((fn) => {
    handler = fn;
    return Promise.resolve(() => {});
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("TemplatesPanel", () => {
  it("groups templates by kind and shows a default selector only for seedable kinds with templates", async () => {
    seed();
    render(<TemplatesPanel project={OPEN_PROJECT} />);

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

  // The whole point of the split: an MCP caller writes to the project scope by default, so the user
  // must see those templates, and see them as the project's — not mixed into the global library.
  it("shows each scope's templates under its own group", async () => {
    seed([summary(1, "daily", "scratchpad")], [summary(2, "sprint", "scratchpad")]);
    render(<TemplatesPanel project={OPEN_PROJECT} />);

    const projectGroup = await screen.findByRole("group", { name: PROJECT_GROUP });
    expect(within(projectGroup).getByRole("button", { name: "sprint" })).toBeTruthy();
    expect(within(projectGroup).queryByRole("button", { name: "daily" })).toBeNull();

    const globalGroup = screen.getByRole("group", { name: GLOBAL_GROUP });
    expect(within(globalGroup).getByRole("button", { name: "daily" })).toBeTruthy();
    expect(within(globalGroup).queryByRole("button", { name: "sprint" })).toBeNull();
  });

  it("gives each scope its own empty state", async () => {
    seed([], []);
    render(<TemplatesPanel project={OPEN_PROJECT} />);

    const globalGroup = await screen.findByRole("group", { name: GLOBAL_GROUP });
    expect(within(globalGroup).getByText("No global templates yet.")).toBeTruthy();
    const projectGroup = screen.getByRole("group", { name: PROJECT_GROUP });
    expect(within(projectGroup).getByText("No templates in this project yet.")).toBeTruthy();
  });

  it("offers only the global group while no project is open", async () => {
    seed([summary(1, "daily", "scratchpad")], [summary(2, "sprint", "scratchpad")]);
    render(<TemplatesPanel project={null} />);

    await screen.findByRole("group", { name: GLOBAL_GROUP });
    expect(screen.queryByRole("group", { name: PROJECT_GROUP })).toBeNull();
    // The other project's rows are not borrowed to fill the missing group.
    expect(screen.queryByRole("button", { name: "sprint" })).toBeNull();
  });

  it("names the scope a new template will land in", async () => {
    seed([], []);
    render(<TemplatesPanel project={OPEN_PROJECT} />);

    const projectGroup = await screen.findByRole("group", { name: PROJECT_GROUP });
    fireEvent.click(within(projectGroup).getByRole("button", { name: /New template/ }));

    expect(await screen.findByText("New scratchpad template in this project")).toBeTruthy();
  });

  it("names the scope of the template being edited", async () => {
    seed([], [summary(2, "sprint", "scratchpad")]);
    read.mockResolvedValue({ ...DAILY, name: "sprint", scope: "project" });
    render(<TemplatesPanel project={OPEN_PROJECT} />);
    fireEvent.click(await screen.findByRole("button", { name: "sprint" }));

    expect(await screen.findByText("Scratchpad template in this project")).toBeTruthy();
  });

  it("duplicates a template from its stored content", async () => {
    seed();
    read.mockResolvedValue(DAILY);
    create.mockResolvedValue({ ...DAILY, id: 9, name: "daily copy", revision: 1 });
    render(<TemplatesPanel project={OPEN_PROJECT} />);
    await screen.findByRole("button", { name: "Duplicate daily" });

    fireEvent.click(screen.getByRole("button", { name: "Duplicate daily" }));
    await waitFor(() => expect(create).toHaveBeenCalled());

    // The copy is only real once it comes back through the library the panel re-reads, so announce
    // the change the way the core does and look for the copy on screen.
    mockLibraries([summary(1, "daily", "scratchpad"), summary(9, "daily copy", "scratchpad")]);
    act(() => handler?.({ type: "TemplateChanged", kind: "scratchpad", project: null }));

    expect(await screen.findByRole("button", { name: "daily copy" })).toBeTruthy();
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
    render(<TemplatesPanel project={OPEN_PROJECT} />);
    const promptGroup = await screen.findByRole("group", { name: "Global prompt templates" });

    fireEvent.click(within(promptGroup).getByRole("button", { name: /New template/ }));
    const createButton = await screen.findByRole("button", { name: /Create template/ });

    fireEvent.change(screen.getByLabelText("Template name"), { target: { value: "review" } });
    fireEvent.change(screen.getByLabelText("Template body"), {
      target: { value: "Review {{pr}}" },
    });
    fireEvent.click(createButton);

    // The form closes on success, returning to the browse list.
    await screen.findByRole("group", { name: "Global prompt templates" });
  });

  it("keeps the editor open and shows the reason when a delete is refused", async () => {
    seed();
    read.mockResolvedValue(DAILY);
    remove.mockRejectedValue("template is locked by another session");
    const deleteButton = await openDaily();

    fireEvent.click(deleteButton);
    fireEvent.click(screen.getByRole("button", { name: "Confirm delete" }));

    await waitFor(() =>
      expect(screen.getByText("template is locked by another session")).toBeTruthy(),
    );
    // Still the editor, not the browse list — a refused delete must not drop the user out of the
    // template they were working on.
    expect(screen.getByRole("heading", { name: "daily" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Scratchpad" })).toBeNull();
  });

  it("returns to the list when a delete succeeds", async () => {
    seed();
    read.mockResolvedValue(DAILY);
    remove.mockResolvedValue(true);
    const deleteButton = await openDaily();

    fireEvent.click(deleteButton);
    fireEvent.click(screen.getByRole("button", { name: "Confirm delete" }));

    await waitFor(() => expect(screen.getByRole("heading", { name: "Scratchpad" })).toBeTruthy());
  });

  it("shows the reason when a duplicate is refused", async () => {
    seed();
    read.mockResolvedValue(DAILY);
    create.mockRejectedValue("name is longer than 200 characters");
    render(<TemplatesPanel project={OPEN_PROJECT} />);
    const duplicate = await screen.findByRole("button", { name: "Duplicate daily" });

    fireEvent.click(duplicate);

    await waitFor(() =>
      expect(screen.getByText("name is longer than 200 characters")).toBeTruthy(),
    );
  });

  it("shows the not-found state when opening a deleted template", async () => {
    seed();
    read.mockRejectedValue("no template under that name");
    render(<TemplatesPanel project={OPEN_PROJECT} />);
    await screen.findByRole("button", { name: "Duplicate daily" });

    // Opening the row (a button named for the template) reads the full template; a rejection surfaces
    // the core's message.
    fireEvent.click(screen.getByRole("button", { name: "daily" }));
    await waitFor(() => expect(screen.getByText("no template under that name")).toBeTruthy());
  });

  it("does not show another project's templates", async () => {
    seed([], [summary(2, "sprint", "scratchpad")]);
    render(<TemplatesPanel project={OTHER_PROJECT} />);

    const projectGroup = await screen.findByRole("group", { name: PROJECT_GROUP });
    expect(within(projectGroup).getByText("No templates in this project yet.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "sprint" })).toBeNull();
  });
});
