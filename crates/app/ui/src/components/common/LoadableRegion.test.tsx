// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LoadableRegion } from "@/components/common/LoadableRegion";
import { failed, loading, ready } from "@/store/loadable";

afterEach(cleanup);

const SKELETON = <div>stand-in row</div>;

describe("LoadableRegion", () => {
  it("shows the stand-in and announces the wait while the first read is in flight", () => {
    const content = vi.fn((value: string) => <p>{value}</p>);
    render(
      <LoadableRegion state={loading<string>()} label="todos" skeleton={SKELETON}>
        {content}
      </LoadableRegion>,
    );

    expect(screen.getByText("stand-in row")).not.toBeNull();
    expect(screen.getByRole("status").getAttribute("aria-busy")).toBe("true");
    expect(screen.getByText("Loading todos")).not.toBeNull();
    expect(content).not.toHaveBeenCalled();
  });

  it("renders the held value with no wrapper of its own once the read lands", () => {
    render(
      <LoadableRegion state={ready("Buy milk")} label="todos" skeleton={SKELETON}>
        {(value) => <p>{value}</p>}
      </LoadableRegion>,
    );

    expect(screen.getByText("Buy milk")).not.toBeNull();
    expect(screen.queryByText("stand-in row")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("offers a retry that re-runs the read once when the first read failed", () => {
    const retry = vi.fn();
    render(
      <LoadableRegion
        state={failed<string>("channel closed")}
        label="todos"
        skeleton={SKELETON}
        onRetry={retry}
      >
        {(value) => <p>{value}</p>}
      </LoadableRegion>,
    );

    expect(screen.getByText("Could not load todos.")).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));

    expect(retry).toHaveBeenCalledTimes(1);
  });

  it("states the failure without an action when there is nothing to re-run", () => {
    render(
      <LoadableRegion state={failed<string>("channel closed")} label="todos" skeleton={SKELETON}>
        {(value) => <p>{value}</p>}
      </LoadableRegion>,
    );

    expect(screen.getByText("Could not load todos.")).not.toBeNull();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
