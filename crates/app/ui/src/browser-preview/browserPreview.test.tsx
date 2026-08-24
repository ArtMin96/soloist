// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";

vi.unmock("@/lib/window");
vi.unmock("@/lib/fileDrop");

vi.mock("@/components/terminal/useTerminal", () => ({
  useTerminal: () => ({ hostRef: { current: null }, state: "not-started" as const }),
}));

import App from "@/App";
import { ptyAttach } from "@/api";
import {
  PREVIEW_IDS,
  PREVIEW_PROCESSES,
  PREVIEW_PTY_MAX_BYTES,
  PREVIEW_PROJECTS,
} from "@/browser-preview/fixture";
import { installBrowserPreview } from "@/browser-preview/install";

function processRow(id: number): HTMLElement {
  const row = document.querySelector<HTMLElement>(`[data-process-id="${id}"]`);
  if (!row) throw new Error(`No preview process row for ${id}`);
  return row;
}

beforeEach(() => installBrowserPreview());

afterEach(async () => {
  cleanup();
  await Promise.resolve();
  clearMocks();
});

describe("browser preview", () => {
  it("installs window and webview metadata before native boundaries are used", async () => {
    const appWindow = getCurrentWindow();
    const webview = getCurrentWebview();

    expect(appWindow.label).toBe("main");
    expect(webview.label).toBe("main");
    await expect(appWindow.isFocused()).resolves.toBe(true);
    await expect(appWindow.isMaximized()).resolves.toBe(false);

    const unlisten = await webview.onDragDropEvent(() => {});
    expect(unlisten).toBeTypeOf("function");
    unlisten();
  });

  it("renders the production App from bounded browser fixtures", async () => {
    render(<App />);

    const rows = await screen.findAllByRole("treeitem");
    expect(rows).toHaveLength(PREVIEW_PROCESSES.length);
    expect(screen.getByText("Soloist")).toBeTruthy();
    for (const project of PREVIEW_PROJECTS) {
      expect(screen.getByText(project.name)).toBeTruthy();
    }
    expect(screen.getAllByText("Agents").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Terminals").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Commands").length).toBeGreaterThan(0);
    expect(
      processRow(PREVIEW_IDS.tests).querySelector("[data-status]")?.getAttribute("data-status"),
    ).toBe("Crashed");
    expect(
      processRow(PREVIEW_IDS.deploy).querySelector("[data-status]")?.getAttribute("data-status"),
    ).toBe("Stopped");
    expect(within(processRow(PREVIEW_IDS.web)).getByText(":1420")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByText(/Browser preview does not support/)).toBeNull();
  });

  it("delivers one bounded terminal resync through the existing Channel path", async () => {
    const frames: { bytes: Uint8Array; resync: boolean }[] = [];

    await ptyAttach(PREVIEW_IDS.terminal, (bytes, resync) => {
      frames.push({ bytes, resync });
    });

    expect(frames).toHaveLength(1);
    expect(frames[0]?.resync).toBe(true);
    expect((frames[0]?.bytes.byteLength ?? PREVIEW_PTY_MAX_BYTES) + 1).toBeLessThanOrEqual(
      PREVIEW_PTY_MAX_BYTES,
    );
    expect(new TextDecoder().decode(frames[0]?.bytes)).toContain(
      "Soloist browser preview fixture ready",
    );
  });
});
