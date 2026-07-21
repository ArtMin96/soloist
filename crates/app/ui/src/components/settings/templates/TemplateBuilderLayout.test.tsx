// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { TemplateBuilderLayout } from "@/components/settings/templates/TemplateBuilderLayout";

afterEach(() => cleanup());

describe("TemplateBuilderLayout", () => {
  it("renders a single column with no divider when there is nothing to preview", () => {
    render(<TemplateBuilderLayout editor={<div>Editor content</div>} preview={null} />);

    expect(screen.getByText("Editor content")).toBeTruthy();
    expect(screen.queryByRole("separator")).toBeNull();
  });

  it("splits editor and preview with a draggable divider between them", () => {
    render(
      <TemplateBuilderLayout
        editor={<div>Editor content</div>}
        preview={<div>Preview content</div>}
      />,
    );

    expect(screen.getByText("Editor content")).toBeTruthy();
    expect(screen.getByText("Preview content")).toBeTruthy();
    expect(screen.getByRole("separator")).toBeTruthy();
  });
});
