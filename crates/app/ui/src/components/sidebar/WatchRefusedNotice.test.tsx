// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { WatchRefusedNotice } from "@/components/sidebar/WatchRefusedNotice";
import type { WatchError } from "@/domain";

afterEach(cleanup);

const REASONS: WatchError[] = ["budget_exhausted", "unwatchable", "unavailable"];

describe("WatchRefusedNotice", () => {
  // The consequence is the load-bearing half: a watch that yields nothing looks exactly like a
  // project nobody is editing, so every refusal has to name what stopped working, not only why.
  it("names what stopped working, whatever the reason", () => {
    for (const reason of REASONS) {
      const { container } = render(<WatchRefusedNotice reason={reason} />);
      const text = container.textContent ?? "";
      expect(text).toContain("restart-on-change");
      expect(text).toContain("git status");
      cleanup();
    }
  });

  // The one refusal the user can do something about is worth nothing unless it names the setting.
  it("names the setting that restores an exhausted watch budget", () => {
    const { container } = render(<WatchRefusedNotice reason="budget_exhausted" />);
    expect(container.textContent).toContain("fs.inotify.max_user_watches");
  });

  it("does not offer a setting to raise when raising one would not help", () => {
    const { container } = render(<WatchRefusedNotice reason="unwatchable" />);
    expect(container.textContent).not.toContain("fs.inotify.max_user_watches");
  });

  it("renders as an advisory strip", () => {
    const { container } = render(<WatchRefusedNotice reason="unavailable" />);
    expect(container.querySelector("[data-advisory-notice]")).toBeTruthy();
  });
});
