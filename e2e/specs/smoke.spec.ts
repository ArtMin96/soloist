import { browser, $ } from "@wdio/globals";

// Proves the harness itself: the built app launches, the embedded WebDriver server accepts the
// session, and the real window renders its shell. No feature behavior — that is what the journey
// specs assert. If this goes red, the harness is broken, not the product.
describe("app shell", () => {
  it("renders the project sidebar on a clean data dir", async () => {
    const projects = await $('nav[aria-label="Projects"]');
    await projects.waitForExist({ timeout: 30_000 });
    expect(await projects.isDisplayed()).toBe(true);
  });

  it("offers the open-project action when no project has been opened", async () => {
    const openProject = await $("aria/Open project");
    await openProject.waitForExist({ timeout: 30_000 });
    expect(await openProject.isDisplayed()).toBe(true);
  });

  it("reports the window title", async () => {
    expect(await browser.getTitle()).toBeDefined();
  });
});
