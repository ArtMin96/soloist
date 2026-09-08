// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";

afterEach(cleanup);

describe("InputGroup", () => {
  it("uses one solid muted pairing when its input is disabled", () => {
    render(
      <InputGroup data-testid="group" data-disabled="true">
        <InputGroupAddon>Search</InputGroupAddon>
        <InputGroupInput aria-label="Search" disabled value="release" readOnly />
      </InputGroup>,
    );

    const groupClasses = screen.getByTestId("group").className.split(/\s+/);
    const addonClasses = screen.getByText("Search").className.split(/\s+/);
    const inputClasses = screen.getByRole("textbox", { name: "Search" }).className.split(/\s+/);

    expect(groupClasses).toEqual(
      expect.arrayContaining([
        "has-disabled:border-border",
        "has-disabled:bg-muted",
        "has-disabled:text-muted-foreground",
      ]),
    );
    expect(groupClasses).not.toContain("has-disabled:opacity-50");
    expect(addonClasses).toContain(
      "group-data-[disabled=true]/input-group:text-muted-foreground",
    );
    expect(addonClasses).not.toContain("group-data-[disabled=true]/input-group:opacity-50");
    expect(inputClasses).toEqual(
      expect.arrayContaining(["disabled:bg-transparent", "disabled:text-muted-foreground"]),
    );
    expect(inputClasses).not.toContain("disabled:opacity-50");
  });
});
