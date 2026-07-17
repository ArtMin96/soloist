import { browser } from "@wdio/globals";
import { sidebar } from "../src/screens/Sidebar.js";
import { titlebar } from "../src/screens/Titlebar.js";

// The window title set in tauri.conf.json and the page title in index.html; nothing retitles it at
// runtime, so the session reporting it proves the driver is attached to the app's own webview.
const APP_TITLE = "Soloist";

// Proves the harness itself: the built app launches, the embedded WebDriver server accepts the
// session, and the real window renders its shell. No feature behavior — that is what the journey
// specs assert. If this goes red, the harness is broken, not the product.
describe("app shell", () => {
  it("renders the project sidebar", async () => {
    await sidebar.waitUntilReady();
  });

  it("offers the open-project action", async () => {
    expect(await titlebar.offersOpenProject()).toBe(true);
  });

  it("reports the window title", async () => {
    expect(await browser.getTitle()).toBe(APP_TITLE);
  });
});
