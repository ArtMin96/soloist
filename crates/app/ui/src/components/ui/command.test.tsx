// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import {
  Command,
  CommandGroup,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "@/components/ui/command";

afterEach(cleanup);

function renderItems(children: ReactNode) {
  render(
    <Command>
      <CommandList>
        <CommandGroup>{children}</CommandGroup>
      </CommandList>
    </Command>,
  );
}

describe("CommandItem", () => {
  // Every row in every palette here is unchecked, and each was carrying an invisible check whose
  // glyph and gap came out of the width its own content had to read in — which is what truncated
  // long branch names, process labels and commands.
  it("puts nothing of its own in a row that is marking nothing chosen", () => {
    renderItems(
      <CommandItem value="feature/a-deliberately-long-branch-name">
        <span>feature/a-deliberately-long-branch-name</span>
      </CommandItem>,
    );

    const row = screen.getByRole("option");
    expect(row.textContent).toBe("feature/a-deliberately-long-branch-name");
    expect(
      row.querySelectorAll("svg").length,
      "the row holds what the caller put in it and nothing else",
    ).toBe(0);
  });

  it("leaves the trailing edge to a shortcut where the row carries one", () => {
    renderItems(
      <CommandItem value="settings">
        <span>Open settings</span>
        <CommandShortcut>Ctrl+,</CommandShortcut>
      </CommandItem>,
    );

    const row = screen.getByRole("option");
    expect(within(row).getByText("Ctrl+,")).toBeTruthy();
  });
});
