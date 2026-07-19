import type { ProjectView } from "@domain";
import { createPromptTemplate } from "../../src/flows/createPromptTemplate.js";
import { openProject } from "../../src/flows/openProject.js";
import { settingsOverlay } from "../../src/screens/SettingsOverlay.js";
import { sidebar } from "../../src/screens/Sidebar.js";
import { templatesPanel } from "../../src/screens/TemplatesPanel.js";

// The process the walk selects so the open project is the one Settings addresses.
const ECHO = "Echo";

// The template the walk fills in. Its body declares two placeholders, so one can be answered while
// the other is left alone — the whole point of the walk. The rendered forms are spelled out rather
// than assembled, so what the user should see is readable at a glance.
const NAME = "release-review";
const DESCRIPTION = "What to ask for before cutting a release";
const BODY = "Review {{diff}} with an eye on {{focus}}.";
const DIFF_VALUE = "the auth patch";
const FOCUS_VALUE = "error handling";
const FOCUS_MARKER = "{{focus}}";
const PARTIALLY_FILLED = `Review ${DIFF_VALUE} with an eye on ${FOCUS_MARKER}.`;
const FULLY_FILLED = `Review ${DIFF_VALUE} with an eye on ${FOCUS_VALUE}.`;

// Filling in a prompt template: the placeholders a prompt declares become value fields, and the
// prompt an agent would receive is shown as the values are typed.
//
// The load-bearing assertion is the unanswered placeholder. Substituting an unanswered fill-in away
// silently was refused in the design — it produces a sentence that reads as complete with its
// subject missing — so a gap must stay visible in the prompt *and* be named above it. The core
// enforces that, but only by treating an absent value as absent: the window is what decides whether
// a value is absent, and an empty field defaulted into the map would answer every placeholder with
// nothing. The headless tests of this run against a hand-written stand-in for the core's render, so
// they prove the window's half against our belief about the core's. This walk is the only thing that
// puts the real renderer and the real window on the same screen, which is where a user reads it.
describe("filling in a prompt template", () => {
  let project: ProjectView;

  before(async () => {
    project = await openProject("basic");
    // The Templates panel addresses whichever project is in view, so the walk puts one there by
    // selecting a process before opening Settings.
    await sidebar.select(ECHO);
    await createPromptTemplate(project.id, NAME, DESCRIPTION, BODY);

    await settingsOverlay.open();
    await settingsOverlay.openTemplates();
    await templatesPanel.openTemplate(NAME);
  });

  it("offers a value field for every placeholder the template declares", async () => {
    // In first-appearance order, and derived by the core from the stored body — the window is never
    // told which placeholders a template has, it reads them back from the write it made.
    expect(await templatesPanel.placeholders()).toEqual(["diff", "focus"]);
  });

  it("leaves an unanswered placeholder in the prompt and names it", async () => {
    await templatesPanel.fill("diff", DIFF_VALUE);

    // The gap stays where the user is already reading...
    const preview = await templatesPanel.waitForPreview(PARTIALLY_FILLED);

    // ...and is named, so it is findable without hunting through a long prompt.
    expect(preview.notices).toHaveLength(1);
    expect(preview.notices[0]).toContain(FOCUS_MARKER);
  });

  it("fills the prompt out once every placeholder is answered", async () => {
    await templatesPanel.fill("focus", FOCUS_VALUE);

    const preview = await templatesPanel.waitForPreview(FULLY_FILLED);
    expect(preview.notices).toEqual([]);
  });

  it("puts a placeholder back when its value is emptied", async () => {
    await templatesPanel.clearValue("focus");

    // An emptied field is not an answer of "". Sending one would substitute the marker away and
    // leave "…with an eye on ." — complete-looking prose with nothing in it, and no longer reported
    // as unanswered, because the core reads a supplied key as answered.
    await templatesPanel.waitForPreview(PARTIALLY_FILLED);
  });

  it("keeps a description cleared after the template is reopened", async () => {
    expect(await templatesPanel.description()).toBe(DESCRIPTION);

    await templatesPanel.clearDescription();
    await templatesPanel.save();
    await templatesPanel.back();

    // Reopening reads the template back from the core, so the description is really gone rather
    // than only absent from the editor that cleared it — the distinction an omitted description,
    // which means "keep the stored one", would hide.
    await templatesPanel.openTemplate(NAME);
    expect(await templatesPanel.description()).toBe("");
  });
});
