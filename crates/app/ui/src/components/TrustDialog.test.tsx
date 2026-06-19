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

    fireEvent.click(screen.getByText("Trust all"));
    expect(onTrustAll).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Not now"));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
