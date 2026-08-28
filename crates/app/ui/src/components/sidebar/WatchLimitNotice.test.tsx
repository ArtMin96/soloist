// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { WatchLimitNotice } from "@/components/sidebar/WatchLimitNotice";
import { WATCH_ERRORS } from "@/domain";

afterEach(cleanup);

describe("WatchLimitNotice", () => {
  // The consequence is the load-bearing half: a watch that yields nothing looks exactly like a
  // project nobody is editing, so every refusal has to name what stopped working, not only why.
  it("names what stopped working, whatever the reason", () => {
    for (const reason of WATCH_ERRORS) {
      const { container } = render(
        <WatchLimitNotice
          limits={{ restarts: { refused: reason }, git_status: { refused: reason } }}
        />,
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
    const { container } = render(
      <WatchLimitNotice limits={{ git_status: { refused: "unavailable" } }} />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("live git status");
    expect(text).not.toContain("restart-on-change");
  });

  it("speaks of one loss in the singular", () => {
    const { container } = render(
      <WatchLimitNotice limits={{ restarts: { refused: "unavailable" } }} />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("has stopped");
    expect(text).not.toContain("have stopped");
  });

  it("speaks of both losses in the plural", () => {
    const { container } = render(
      <WatchLimitNotice
        limits={{ restarts: { refused: "unavailable" }, git_status: { refused: "unavailable" } }}
      />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("have stopped");
    expect(text).not.toContain("it has stopped");
  });

  // Both purposes watch the same tree and usually fail together for the same reason. Stating that
  // reason twice would read as two separate faults.
  it("states a shared cause once", () => {
    const { container } = render(
      <WatchLimitNotice
        limits={{ restarts: { refused: "unwatchable" }, git_status: { refused: "unwatchable" } }}
      />,
    );
    const text = container.textContent ?? "";
    expect(text.split("could not be read")).toHaveLength(2);
  });

  it("states each cause when the two purposes were refused differently", () => {
    const { container } = render(
      <WatchLimitNotice
        limits={{
          restarts: { refused: "budget_exhausted" },
          git_status: { refused: "unwatchable" },
        }}
      />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("fs.inotify.max_user_watches");
    expect(text).toContain("could not be read");
  });

  // The one refusal the user can do something about is worth nothing unless it names the setting.
  it("names the setting that restores an exhausted watch budget", () => {
    const { container } = render(
      <WatchLimitNotice limits={{ restarts: { refused: "budget_exhausted" } }} />,
    );
    expect(container.textContent).toContain("fs.inotify.max_user_watches");
  });

  it("does not offer a setting to raise when raising one would not help", () => {
    const { container } = render(
      <WatchLimitNotice limits={{ restarts: { refused: "unwatchable" } }} />,
    );
    expect(container.textContent).not.toContain("fs.inotify.max_user_watches");
  });

  it("renders as an advisory strip that waits its turn to be announced", () => {
    const { container } = render(
      <WatchLimitNotice limits={{ restarts: { refused: "unavailable" } }} />,
    );
    const notice = container.querySelector("[data-advisory-notice]");
    expect(notice?.getAttribute("role")).toBe("status");
  });

  // Degraded is not stopped: the repository's own state stays watched, and the notice has to say
  // what is still true or it reads as a bigger failure than it is.
  it("says what a degraded git status still follows, and does not claim it stopped", () => {
    const { container } = render(<WatchLimitNotice limits={{ git_status: "degraded" }} />);
    const text = container.textContent ?? "";
    expect(text).toContain("Live git status still follows commits and staging");
    expect(text).not.toContain("stopped");
  });

  it("names both purposes when both are degraded", () => {
    const { container } = render(
      <WatchLimitNotice limits={{ restarts: "degraded", git_status: "degraded" }} />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("Live git status still follows commits and staging");
    expect(text).toContain("Restart-on-change only sees the directories your patterns name");
  });

  // A project may lose one purpose entirely while the other is only reduced — the two conditions
  // must read as two distinct sentences, not one merged claim.
  it("states a refusal and a degradation as two distinct sentences", () => {
    const { container } = render(
      <WatchLimitNotice
        limits={{ restarts: { refused: "unwatchable" }, git_status: "degraded" }}
      />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain(
      "Not watching this project's files for restart-on-change, so it has stopped.",
    );
    expect(text).toContain("Live git status still follows commits and staging");
  });

  // The refused/degraded split is per purpose, not per project — a project with one purpose
  // refused reads "it has stopped", never "they have", even though something else is also limited.
  it("keeps the refused count to the refused purposes alone", () => {
    const { container } = render(
      <WatchLimitNotice
        limits={{ restarts: { refused: "unwatchable" }, git_status: "degraded" }}
      />,
    );
    const text = container.textContent ?? "";
    expect(text).toContain("it has stopped");
    expect(text).not.toContain("they have stopped");
  });

  // A purely degraded project is informational rather than urgent — it reads as a calmer, neutral
  // panel, not the amber strip a refusal wears.
  it("reads as a calmer strip when nothing is refused", () => {
    const { container } = render(<WatchLimitNotice limits={{ git_status: "degraded" }} />);
    const notice = container.querySelector("[data-advisory-notice]");
    expect(notice?.className).toContain("border-border");
    expect(notice?.className).toContain("bg-card");
    expect(notice?.className).not.toContain("status-transition");
  });

  it("keeps the urgent tone when any purpose is refused, even alongside a degradation", () => {
    const { container } = render(
      <WatchLimitNotice
        limits={{ restarts: { refused: "unwatchable" }, git_status: "degraded" }}
      />,
    );
    const notice = container.querySelector("[data-advisory-notice]");
    expect(notice?.className).toContain("status-transition");
  });
});
