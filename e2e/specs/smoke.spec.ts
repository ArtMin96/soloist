import { browser, $ } from "@wdio/globals";

// Proves the harness itself: the built app launches, the embedded WebDriver server accepts the
// session, and the real window renders its shell. No feature behavior — that is what the journey
// specs assert. If this goes red, the harness is broken, not the product.
//
// It asserts only on what is true however the app got here. The app process is shared across spec
// files in a run, so whether a project is open depends on which specs ran first; a smoke test that
// assumed an empty app would pass or fail on spec ordering.
describe("app shell", () => {
  it("renders the project sidebar", async () => {
    const projects = await $('nav[aria-label="Projects"]');
    await projects.waitForExist({ timeout: 30_000 });
    expect(await projects.isDisplayed()).toBe(true);
  });

  it("offers the open-project action", async () => {
    const openProject = await $("aria/Open project");
    await openProject.waitForExist({ timeout: 30_000 });
    expect(await openProject.isDisplayed()).toBe(true);
  });

  it("reports the window title", async () => {
    expect(await browser.getTitle()).toBeDefined();
  });
});
