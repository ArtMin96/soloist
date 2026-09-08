// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ScratchpadCreateForm } from "@/components/orchestration/ScratchpadCreateForm";
import type { SaveOutcome } from "@/store/saveOutcome";

// The rich editor needs real layout jsdom does not provide; a textarea standing in for it keeps this
// file on what the form does with a name and a draft rather than on TipTap.
vi.mock("@/components/editor/LazyRichTextEditor", () => ({
  LazyRichTextEditor: (props: { ariaLabel?: string; onChange: (value: string) => void }) => (
    <textarea
      aria-label={props.ariaLabel}
      onChange={(event) => props.onChange(event.target.value)}
    />
  ),
}));

afterEach(cleanup);

const NAME_FIELD = "New scratchpad name";
const REFUSAL = "a scratchpad named release-plan already exists";

/** The board's half of the exchange: a refusal becomes the error the form is asked to show. */
function Board({ onCreate }: { onCreate: (name: string, body: string) => Promise<SaveOutcome> }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <ScratchpadCreateForm
      onCancel={() => {}}
      error={error}
      onCreate={async (name, body) => {
        const outcome = await onCreate(name, body);
        setError(outcome === "refused" ? REFUSAL : null);
        return outcome;
      }}
    />
  );
}

const nameField = () => screen.getByLabelText(NAME_FIELD) as HTMLInputElement;
const createButton = () => screen.getByRole("button", { name: /Creat/ }) as HTMLButtonElement;

describe("ScratchpadCreateForm", () => {
  // A scratchpad is addressed by its name, so there is nothing to create without one — and the core
  // would refuse it, which is a round trip the reader should never have to make to learn that.
  it("offers no Create until the scratchpad has a name", () => {
    render(<Board onCreate={vi.fn(async () => "saved" as const)} />);
    expect(createButton().disabled).toBe(true);

    fireEvent.change(nameField(), { target: { value: "   " } });
    expect(createButton().disabled).toBe(true);

    fireEvent.change(nameField(), { target: { value: "release-plan" } });
    expect(createButton().disabled).toBe(false);
  });

  // A refused create must cost the author nothing: the name they typed stays where it was, with the
  // core's reason above it, so they can adjust it instead of retyping the draft.
  it("keeps the typed name and names the refusal when the core turns a create down", async () => {
    render(<Board onCreate={vi.fn(async () => "refused" as const)} />);

    fireEvent.change(nameField(), { target: { value: "release-plan" } });
    fireEvent.change(screen.getByLabelText("New scratchpad body"), {
      target: { value: "the plan" },
    });
    fireEvent.click(createButton());

    await waitFor(() => expect(screen.getByText(REFUSAL)).toBeTruthy());
    expect(nameField().value).toBe("release-plan");
    expect(createButton().disabled).toBe(false);
  });

  it("posts the draft once, however many times Create is pressed", async () => {
    const onCreate = vi.fn(async () => "saved" as const);
    render(<Board onCreate={onCreate} />);

    fireEvent.change(nameField(), { target: { value: "release-plan" } });
    fireEvent.change(screen.getByLabelText("New scratchpad body"), {
      target: { value: "the plan" },
    });
    fireEvent.click(createButton());
    fireEvent.click(createButton());

    await waitFor(() => expect(onCreate).toHaveBeenCalledTimes(1));
    expect(onCreate).toHaveBeenCalledWith("release-plan", "the plan");
  });
});
