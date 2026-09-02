// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { Section } from "@/components/common/Section";

afterEach(cleanup);

describe("Section", () => {
  it("names its region by its title", () => {
    render(<Section title="Blockers">body</Section>);
    // A <section> only takes the region role once it has an accessible name, so finding it by
    // name proves the heading is both rendered and wired to the region.
    expect(screen.getByRole("region", { name: "Blockers" })).toBeTruthy();
  });

  it("names each region separately when several are on the page", () => {
    render(
      <>
        <Section title="Details">one</Section>
        <Section title="Comments">two</Section>
      </>,
    );
    // Two regions sharing one heading id would leave both answering to the same name.
    expect(screen.getByRole("region", { name: "Details" }).textContent).toBe("Detailsone");
    expect(screen.getByRole("region", { name: "Comments" }).textContent).toBe("Commentstwo");
  });

  it("renders the aside and the action alongside the title", () => {
    render(
      <Section title="Commands" aside={3} action={<button type="button">Add</button>}>
        body
      </Section>,
    );
    const region = screen.getByRole("region", { name: "Commands" });
    expect(region.textContent).toContain("3");
    expect(screen.getByRole("button", { name: "Add" })).toBeTruthy();
  });
});
