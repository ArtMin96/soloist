// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { WatchRefusedNotice } from "@/components/sidebar/WatchRefusedNotice";
import { WATCH_ERRORS } from "@/domain";

afterEach(cleanup);

describe("WatchRefusedNotice", () => {
  // The consequence is the load-bearing half: a watch that yields nothing looks exactly like a
  // project nobody is editing, so every refusal has to name what stopped working, not only why.
  it("names what stopped working, whatever the reason", () => {
    for (const reason of WATCH_ERRORS) {
      const { container } = render(
        <WatchRefusedNotice refusals={{ restarts: reason, git_status: reason }} />,
      );
      const text = container.textContent ?? "";
      expect(text).toContain("restart-on-change");
      expect(text).toContain("git status");
      cleanup();
    }
  });

  // A project whose commands declare no globs never asks for a restart watch, so it is the ordinary
  // project that a git-only refusal would be lying to by claiming restart-on-change stopped.
  it("claims only the loss of a purpose that was actually refused", () => {
    const { container } = render(<WatchRefusedNotice refusals={{ git_status: "unavailable" }} />);
    const text = container.textContent ?? "";
    expect(text).toContain("live git status");
    expect(text).not.toContain("restart-on-change");
  });

  it("speaks of one loss in the singular", () => {
    const { container } = render(<WatchRefusedNotice refusals={{ restarts: "unavailable" }} />);
    const text = container.textContent ?? "";
    expect(text).toContain("has stopped");
    expect(text).not.toContain("have stopped");
  });

  it("speaks of both losses in the plural", () => {
    const { container } = render(
      <WatchRefusedNotice refusals={{ restarts: "unavailable", git_status: "unavailable" }} />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("have stopped");
    expect(text).not.toContain("it has stopped");
  });

  // Both purposes watch the same tree and usually fail together for the same reason. Stating that
  // reason twice would read as two separate faults.
  it("states a shared cause once", () => {
    const { container } = render(
      <WatchRefusedNotice refusals={{ restarts: "unwatchable", git_status: "unwatchable" }} />,
    );
    const text = container.textContent ?? "";
    expect(text.split("could not be read")).toHaveLength(2);
  });

  it("states each cause when the two purposes were refused differently", () => {
    const { container } = render(
      <WatchRefusedNotice refusals={{ restarts: "budget_exhausted", git_status: "unwatchable" }} />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("fs.inotify.max_user_watches");
    expect(text).toContain("could not be read");
  });

  // The one refusal the user can do something about is worth nothing unless it names the setting.
  it("names the setting that restores an exhausted watch budget", () => {
    const { container } = render(
      <WatchRefusedNotice refusals={{ restarts: "budget_exhausted" }} />,
    );
    expect(container.textContent).toContain("fs.inotify.max_user_watches");
  });

  it("does not offer a setting to raise when raising one would not help", () => {
    const { container } = render(<WatchRefusedNotice refusals={{ restarts: "unwatchable" }} />);
    expect(container.textContent).not.toContain("fs.inotify.max_user_watches");
  });

  // The condition persists rather than happening to the user, so the strip waits for a pause in
  // speech instead of interrupting whatever the reader is on.
  it("renders as an advisory strip that waits its turn to be announced", () => {
    const { container } = render(<WatchRefusedNotice refusals={{ restarts: "unavailable" }} />);
    const notice = container.querySelector("[data-advisory-notice]");
    expect(notice?.getAttribute("role")).toBe("status");
  });
});
