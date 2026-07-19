// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { TemplateEditorBody } from "@/components/settings/templates/TemplateEditorBody";

// Stub the lazy rich editor so this test never mounts TipTap; the body it reports is what gets copied.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { ariaLabel?: string; onChange: (v: string) => void }) => (
    <textarea
      aria-label={props.ariaLabel}
      onChange={(event) => props.onChange(event.target.value)}
    />
  ),
}));

const BODY = "Review {{diff}} with an eye on {{focus}}.";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("TemplateEditorBody copy", () => {
  // The copied text is the artifact the user pastes, so its exact content is the behaviour. A
  // heading naming the template used to be prepended, which corrupted every paste: a prompt handed
  // to an agent gained a title it never declared, and a seedable template gained an H1 that is not
  // part of the document it seeds.
  it("copies the template body verbatim, with nothing prepended", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    render(
      <TemplateEditorBody
        initialBody={BODY}
        initialDescription=""
        onSave={() => Promise.resolve()}
        paused={false}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Copy Markdown/ }));

    await waitFor(() => expect(writeText).toHaveBeenCalledWith(BODY));
  });

  it("copies the edited body, not the body it mounted with", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    render(
      <TemplateEditorBody
        initialBody={BODY}
        initialDescription=""
        onSave={() => Promise.resolve()}
        paused={false}
      />,
    );

    fireEvent.change(screen.getByLabelText("Template body"), {
      target: { value: "Summarize {{session}}." },
    });
    fireEvent.click(screen.getByRole("button", { name: /Copy Markdown/ }));

    await waitFor(() => expect(writeText).toHaveBeenCalledWith("Summarize {{session}}."));
  });
});
