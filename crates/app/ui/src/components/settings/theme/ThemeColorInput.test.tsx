// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ThemeColorInput } from "@/components/settings/theme/ThemeColorInput";

afterEach(cleanup);

describe("ThemeColorInput", () => {
  it("reflects an external value change into the draft", () => {
    const { rerender } = render(
      <ThemeColorInput label="Accent" value="#112233" onChange={vi.fn()} />,
    );
    expect(screen.getByLabelText("Accent")).toHaveProperty("value", "#112233");

    rerender(<ThemeColorInput label="Accent" value="#445566" onChange={vi.fn()} />);

    expect(screen.getByLabelText("Accent")).toHaveProperty("value", "#445566");
  });

  it("keeps an in-progress edit across a re-render that leaves value unchanged", () => {
    const { rerender } = render(
      <ThemeColorInput label="Accent" value="#112233" onChange={vi.fn()} />,
    );

    fireEvent.change(screen.getByLabelText("Accent"), { target: { value: "#4" } });
    expect(screen.getByLabelText("Accent")).toHaveProperty("value", "#4");

    // A re-render the parent triggers for an unrelated reason must not clobber the draft the user
    // is mid-typing, as long as `value` itself has not moved.
    rerender(<ThemeColorInput label="Accent" value="#112233" onChange={vi.fn()} />);

    expect(screen.getByLabelText("Accent")).toHaveProperty("value", "#4");
  });
});
