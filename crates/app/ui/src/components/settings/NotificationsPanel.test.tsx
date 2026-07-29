// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { NotificationsPanel } from "@/components/settings/NotificationsPanel";
import type { Notifications, NotifierStatus } from "@/domain";

afterEach(() => {
  cleanup();
  clearMocks();
});

const LISTENING: NotifierStatus = {
  type: "available",
  server: "gnome-shell",
  version: "46.0",
  capabilities: ["body", "sound"],
};

/** Serves the whole tab's reads, collecting what each save was handed. */
function mountWith(stored: Notifications, status: NotifierStatus = { type: "unavailable" }) {
  const saved: Notifications[] = [];
  const sent: string[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "notification_settings") return stored;
    if (cmd === "set_notification_settings") {
      const next = (args as { notifications: Notifications }).notifications;
      saved.push(next);
      return next;
    }
    if (cmd === "notifier_status") return status;
    if (cmd === "send_test_notification") sent.push(cmd);
    return undefined;
  });
  render(<NotificationsPanel />);
  return { saved, sent };
}

const masterSwitch = () => screen.getByRole("switch", { name: "Show notifications" });
const soundPicker = () => screen.getByRole("combobox", { name: "Alert sound" });

describe("Settings — Notifications", () => {
  it("loads the stored master switch and binds it into the toggle", async () => {
    mountWith({ enabled: false, bell: null });

    // A panel that dropped the loaded value would render the default-on switch; the stored `false`
    // must win, so the toggle reads off.
    await waitFor(() => expect(masterSwitch().getAttribute("aria-checked")).toBe("false"));
  });

  it("persists a toggle without dropping the chosen sound", async () => {
    const { saved } = mountWith({ enabled: true, bell: "bell" });
    await waitFor(() => expect(masterSwitch().getAttribute("aria-checked")).toBe("true"));

    fireEvent.click(masterSwitch());

    // The setter replaces the whole document, so a row that sent only its own field would silently
    // reset the other — turning notifications off and on again would lose the user's sound.
    await waitFor(() => expect(saved).toEqual([{ enabled: false, bell: "bell" }]));
  });

  it("shows no sound chosen as None rather than as the first sound offered", async () => {
    mountWith({ enabled: true, bell: null });

    // `null` is a real choice — silence — not an absent one. A picker that fell through to its
    // first item would tell a user who wants silence that they had picked a bell, and every alert
    // afterwards would contradict the tab they were reading.
    await waitFor(() => expect(soundPicker().textContent).toBe("None"));
  });

  it("binds a stored sound into the picker by its label", async () => {
    mountWith({ enabled: true, bell: "dialog-warning" });

    await waitFor(() => expect(soundPicker().textContent).toBe("Warning"));
  });

  it("names the desktop service that is listening", async () => {
    mountWith({ enabled: true, bell: null }, LISTENING);

    await waitFor(() => expect(screen.getByText("Connected")).toBeTruthy());
    expect(screen.getByText("gnome-shell 46.0")).toBeTruthy();
  });

  it("scopes an unavailable desktop channel to the desktop, leaving the settings operable", async () => {
    const { saved } = mountWith({ enabled: true, bell: null }, { type: "unavailable" });

    await waitFor(() => expect(screen.getByText("Not available")).toBeTruthy());
    // The master switch and the sound also govern in-app toasts, which never touch the desktop
    // service — so the row must say what is actually unreachable, and the controls must keep
    // working. Presenting them as dead would be the confusion this row exists to prevent.
    expect(screen.getByText(/In-app toasts still will/)).toBeTruthy();

    expect(masterSwitch().getAttribute("aria-disabled")).not.toBe("true");
    fireEvent.click(masterSwitch());
    await waitFor(() => expect(saved).toEqual([{ enabled: false, bell: null }]));
  });

  it("reports a test alert as sent, never as delivered", async () => {
    const { sent } = mountWith({ enabled: true, bell: null }, LISTENING);
    await waitFor(() => expect(screen.getByText("Connected")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: "Send test" }));

    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Sent"));
    expect(sent).toEqual(["send_test_notification"]);
    // Showing a desktop notification discards the desktop's answer, so whether one arrived is not
    // knowable. A word implying it did would be the app inventing a fact about the user's screen.
    expect(document.body.textContent).not.toMatch(/deliver/i);
  });
});
