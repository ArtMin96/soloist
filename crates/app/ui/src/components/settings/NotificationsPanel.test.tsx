// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { NotificationsPanel } from "@/components/settings/NotificationsPanel";
import type { Notifications } from "@/domain";

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Settings — Notifications", () => {
  it("loads the stored master switch and binds it into the toggle", async () => {
    const stored: Notifications = { enabled: false };
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === "notification_settings") return stored;
      return undefined;
    });

    render(<NotificationsPanel />);

    await waitFor(() => expect(calls).toContain("notification_settings"));

    // A panel that dropped the loaded value would render the default-on switch; the stored `false`
    // must win, so the toggle reads off.
    const toggle = screen.getByRole("switch", { name: "Desktop notifications" });
    await waitFor(() => expect(toggle.getAttribute("aria-checked")).toBe("false"));
  });

  it("persists a toggle through the whole-document facade setter", async () => {
    let saved: Notifications | null = null;
    mockIPC((cmd, args) => {
      if (cmd === "notification_settings") return { enabled: true } satisfies Notifications;
      if (cmd === "set_notification_settings") {
        saved = (args as { notifications: Notifications }).notifications;
        return saved;
      }
      return undefined;
    });

    render(<NotificationsPanel />);

    const toggle = screen.getByRole("switch", { name: "Desktop notifications" });
    await waitFor(() => expect(toggle.getAttribute("aria-checked")).toBe("true"));

    fireEvent.click(toggle);

    await waitFor(() => expect(saved).toEqual({ enabled: false }));
  });
});
