// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { RenderedPrompt, TemplateView } from "@/domain";
import { TemplatesPanel } from "@/components/settings/TemplatesPanel";

vi.mock("@/api", () => ({
  templates: vi.fn(),
  templateDefaults: vi.fn(),
  setDefaultTemplate: vi.fn(),
  templateCreate: vi.fn(),
  templateDelete: vi.fn(),
  templateRead: vi.fn(),
  templateUpdate: vi.fn(),
  templateRender: vi.fn(),
  onDomainEvent: vi.fn(() => Promise.resolve(() => {})),
}));

// Stub the lazy rich editor so this test never mounts TipTap — the preview is what is under test.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { ariaLabel?: string; onChange: (v: string) => void }) => (
    <textarea
      aria-label={props.ariaLabel}
      onChange={(event) => props.onChange(event.target.value)}
    />
  ),
}));

import {
  templateDefaults,
  templateRender,
  templates as listTemplates,
  templateRead,
  templateUpdate,
} from "@/api";

const list = vi.mocked(listTemplates);
const defaults = vi.mocked(templateDefaults);
const read = vi.mocked(templateRead);
const render_ = vi.mocked(templateRender);
const update = vi.mocked(templateUpdate);

const OPEN_PROJECT = 7;

const REVIEW: TemplateView = {
  id: 1,
  kind: "prompt",
  name: "review",
  description: null,
  body: "Review {{diff}} with an eye on {{focus}}.",
  placeholders: ["diff", "focus"],
  scope: "global",
  revision: 3,
};

// The placeholder names a body declares, in first-appearance order — what the core derives on every
// read and hands back on every write, and never something the UI works out for itself.
function declaredIn(body: string): string[] {
  return [...body.matchAll(/\{\{(\w+)\}\}/g)].map((match) => match[1]).filter(unique);
}

function unique<T>(value: T, index: number, all: T[]): boolean {
  return all.indexOf(value) === index;
}

// Stands in for the core's render across the IPC boundary, honoring the contract the UI depends on:
// a placeholder with a supplied value is substituted, one without keeps its literal marker and is
// reported unfilled, and a supplied name the body never declares is reported unknown. A key being
// *present* is what counts as supplied — the distinction the "clear a field" behaviour rides on.
function coreRender(body: string, values: Record<string, string>): RenderedPrompt {
  const declared = declaredIn(body);
  return {
    text: body.replace(/\{\{(\w+)\}\}/g, (raw, name: string) =>
      name in values ? values[name] : raw,
    ),
    unfilled: declared.filter((name) => !(name in values)),
    unknown: Object.keys(values).filter((name) => !declared.includes(name)),
  };
}

// Backs the panel with one global prompt template held in memory, so a save moves the stored body,
// its declared placeholders, and its revision the way the core would — and every later render reads
// the body that is actually stored.
function seed(template: TemplateView = REVIEW) {
  let stored = template;
  list.mockImplementation((kind, project) =>
    Promise.resolve(kind === stored.kind && project == null ? [stored] : []),
  );
  defaults.mockResolvedValue({ scratchpad: null, todo: null });
  read.mockImplementation(() => Promise.resolve(stored));
  update.mockImplementation((_kind, _project, _name, description, body) => {
    stored = {
      ...stored,
      description: description === null ? stored.description : description || null,
      body,
      placeholders: declaredIn(body),
      revision: stored.revision + 1,
    };
    return Promise.resolve(stored);
  });
  render_.mockImplementation((_project, _name, values) =>
    Promise.resolve(coreRender(stored.body, values)),
  );
}

// Drills from the browse list into the editor for the seeded prompt template.
async function openTemplate(template: TemplateView = REVIEW) {
  seed(template);
  render(<TemplatesPanel project={OPEN_PROJECT} />);
  fireEvent.click(await screen.findByRole("button", { name: template.name }));
  await screen.findByRole("button", { name: "Delete template" });
}

// The rendered prompt is the only <pre> on the surface.
function output(): string {
  const pre = document.querySelector("pre");
  return pre?.textContent ?? "";
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("template preview", () => {
  it("renders the prompt with the value typed for each placeholder", async () => {
    await openTemplate();

    fireEvent.change(screen.getByLabelText("Value for diff"), {
      target: { value: "the auth patch" },
    });
    fireEvent.change(screen.getByLabelText("Value for focus"), {
      target: { value: "error handling" },
    });

    await waitFor(() =>
      expect(output()).toBe("Review the auth patch with an eye on error handling."),
    );
  });

  it("leaves an unfilled placeholder literal in the prompt and names it in a notice", async () => {
    await openTemplate();

    fireEvent.change(screen.getByLabelText("Value for diff"), {
      target: { value: "the auth patch" },
    });

    // The gap is visible where the user is already reading...
    await waitFor(() => expect(output()).toBe("Review the auth patch with an eye on {{focus}}."));
    // ...and named, so it is findable without hunting through a long prompt.
    const notice = await screen.findByText(/No value for \{\{focus\}\}/);
    // A polite live region, not an assertive one: this advisory re-renders as the user types, and
    // `alert` would re-interrupt a screen reader on every keystroke.
    expect(notice.closest("[role='status']")).toBeTruthy();
  });

  it("puts the marker back when a filled value is cleared", async () => {
    await openTemplate();
    const diff = screen.getByLabelText("Value for diff");

    fireEvent.change(diff, { target: { value: "the auth patch" } });
    await waitFor(() => expect(output()).toContain("the auth patch"));

    fireEvent.change(diff, { target: { value: "" } });

    // An emptied field is not an answer of "": substituting it away would leave "Review  with an
    // eye on…", which reads as complete while its subject is missing.
    await waitFor(() => expect(output()).toContain("Review {{diff}}"));
    expect(await screen.findByText(/No value for \{\{diff\}\}/)).toBeTruthy();
  });

  it("reports a supplied value that matches no placeholder", async () => {
    // The body declares only {{diff}}, but the template still lists a placeholder the user can type
    // a value for — the shape left behind when a marker is edited out of a body mid-session.
    await openTemplate({ ...REVIEW, body: "Review {{diff}}.", placeholders: ["diff", "focus"] });

    fireEvent.change(screen.getByLabelText("Value for focus"), {
      target: { value: "error handling" },
    });

    const notice = await screen.findByText(/\{\{focus\}\} matches no placeholder/);
    // A polite live region, not an assertive one: this advisory re-renders as the user types, and
    // `alert` would re-interrupt a screen reader on every keystroke.
    expect(notice.closest("[role='status']")).toBeTruthy();
    // The stray value changes nothing about the prompt itself.
    await waitFor(() => expect(output()).toBe("Review {{diff}}."));
  });

  it("needs no value fields for a template that declares no placeholders, and previews it as written", async () => {
    await openTemplate({ ...REVIEW, body: "Summarize the session.", placeholders: [] });

    expect(await screen.findByText(/declares no placeholders/)).toBeTruthy();
    expect(screen.queryByLabelText(/^Value for /)).toBeNull();
    await waitFor(() => expect(output()).toBe("Summarize the session."));
  });

  it("offers a value field for a placeholder added to the body once the edit is saved", async () => {
    await openTemplate({ ...REVIEW, body: "Review {{diff}}.", placeholders: ["diff"] });
    expect(screen.queryByLabelText("Value for scope")).toBeNull();

    fireEvent.change(screen.getByLabelText("Template body"), {
      target: { value: "Review {{diff}} at {{scope}}." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    // The declared list is the core's, re-read from the write it accepted — so a marker typed into
    // the body becomes a fill-in the moment the edit lands, not on the next time the panel is opened.
    expect(await screen.findByLabelText("Value for scope")).toBeTruthy();
    await waitFor(() => expect(output()).toBe("Review {{diff}} at {{scope}}."));
  });

  it("offers no preview for a kind that is never rendered", async () => {
    await openTemplate({
      ...REVIEW,
      kind: "scratchpad",
      name: "daily",
      body: "## Plan {{day}}",
      placeholders: ["day"],
    });

    expect(screen.queryByRole("heading", { name: "Preview" })).toBeNull();
    expect(screen.queryByLabelText("Value for day")).toBeNull();
  });
});
