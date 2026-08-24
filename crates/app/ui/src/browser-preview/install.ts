import { isTauri } from "@tauri-apps/api/core";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createBrowserPreviewDispatcher } from "@/browser-preview/fixture";

/** Installs the development-only Tauri host used by the browser preview. */
export function installBrowserPreview(): void {
  if (isTauri()) return;

  mockWindows("main");
  mockIPC(createBrowserPreviewDispatcher(), { shouldMockEvents: true });
}

installBrowserPreview();
