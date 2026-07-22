// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TrustDialog } from "@/components/TrustDialog";
import type { TrustReview } from "@/store/useTrust";

const REVIEW: TrustReview = {
  project: 1,
  diff: { added: ["Api"], updated: [], removed: [], renamed: [] },
  commands: [{ name: "Api", command: "cargo run", working_dir: "api", env: { PORT: "4000" } }],
};

afterEach(cleanup);

describe("TrustDialog", () => {
  it("stays closed when there is no review", () => {
    render(
      <TrustDialog
        review={null}
        onTrustCommand={() => {}}
        onTrustAll={() => {}}
        onDismiss={() => {}}
      />,
    );
    expect(screen.queryByText("Trust changed commands")).toBeNull();
  });

  it("shows each command's detail and the change summary", () => {
    render(
      <TrustDialog
        review={REVIEW}
        onTrustCommand={() => {}}
        onTrustAll={() => {}}
        onDismiss={() => {}}
      />,
    );
    expect(screen.getByText("Trust changed commands")).toBeTruthy();
    expect(screen.getByText("Added")).toBeTruthy();
    expect(screen.getByText("cargo run")).toBeTruthy();
    expect(screen.getByText("in api")).toBeTruthy();
    expect(screen.getByText("PORT=4000")).toBeTruthy();
    expect(screen.getByLabelText("Trust Api")).toBeTruthy();
  });

  it("asks about one command without claiming the file changed", () => {
    render(
      <TrustDialog
        review={{ ...REVIEW, diff: null }}
        onTrustCommand={() => {}}
        onTrustAll={() => {}}
        onDismiss={() => {}}
      />,
    );

    // Reached from the sidebar affordance, not from a solo.yml change: the copy says what
    // is true, and there is no change summary to show.
    expect(screen.getByText("Trust this command")).toBeTruthy();
    expect(screen.queryByText("Added")).toBeNull();
    expect(screen.getByText("cargo run")).toBeTruthy();
  });

  it("gives every environment pair its own line", () => {
    render(
      <TrustDialog
        review={{
          ...REVIEW,
          commands: [
            {
              name: "Api",
              command: "cargo run",
              working_dir: null,
              env: { PORT: "4000", LD_PRELOAD: "/tmp/evil.so" },
            },
          ],
        }}
        onTrustCommand={() => {}}
        onTrustAll={() => {}}
        onDismiss={() => {}}
      />,
    );

    // Joined into one line, a trailing pair is the first thing a single row's overflow
    // hides — the point of the review is that the last pair reads as plainly as the first.
    expect(screen.getByText("PORT=4000")).toBeTruthy();
    expect(screen.getByText("LD_PRELOAD=/tmp/evil.so")).toBeTruthy();
  });

  it("routes trust decisions to their callbacks", () => {
    const onTrustCommand = vi.fn();
    const onTrustAll = vi.fn();
    const onDismiss = vi.fn();
    render(
      <TrustDialog
        review={REVIEW}
        onTrustCommand={onTrustCommand}
        onTrustAll={onTrustAll}
        onDismiss={onDismiss}
      />,
    );

    fireEvent.click(screen.getByLabelText("Trust Api"));
    expect(onTrustCommand).toHaveBeenCalledWith("Api");

    // One command under review, so the primary action grants that one rather than "all".
    fireEvent.click(screen.getByRole("button", { name: "Trust" }));
    expect(onTrustAll).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Not now"));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
