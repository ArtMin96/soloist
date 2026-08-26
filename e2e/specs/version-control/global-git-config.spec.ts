import { openProject } from "../../src/flows/openProject.js";
import { CONFIGURED_IGNORE } from "../../src/harness/gitConfig.js";
import {
  addUntrackedFiles,
  makeRepository,
} from "../../src/harness/repository.js";
import { gitRail } from "../../src/screens/GitRail.js";
import { sidebar } from "../../src/screens/Sidebar.js";

// The change that makes the fixture a repository worth reading. Not what this walk is about; the
// rail says nothing at all about a project until its first status read has answered.
const CHANGED_PATH = "notes.md";

// Two untracked files side by side in the same folder, alike in every way the app can see — same
// depth, same extension, neither tracked, neither named anywhere in the project. The only thing
// that tells them apart is the global git configuration the run gives the app.
const LISTED = "review-note.md";

// The fixture process in front of the reader: the rail shows the repository the selected row
// belongs to.
const SELECTED = "Echo";

// Soloist reads a repository by running the user's own `git`, so their configuration is part of
// every answer it gets — a path their global `core.excludesFile` names comes back ignored, and
// nothing in the app decides that. Only a real window over a real core can show it: the adapter is
// a subprocess and the excludes file is read by `git` itself, so no test that stands in for either
// reaches the behaviour at all.
//
// It is also how the suite proves its own containment. The app under test is given a global git
// configuration belonging to the run, and this is what shows the app really received it — so the
// developer's own `~/.gitconfig`, and any credential helper named in it, reach neither the harness
// nor the app under test.
describe("the project's files under the user's own git configuration", () => {
  before(async () => {
    await openProject("basic", (root) => {
      makeRepository(root, {
        path: CHANGED_PATH,
        committed: "# Notes\n",
        working: "# Notes\n\nStill being written.\n",
      });
      addUntrackedFiles(root, {
        [LISTED]: "kept in the listing\n",
        [CONFIGURED_IGNORE]: "excluded by the run's own git configuration\n",
      });
    });
    await sidebar.select(SELECTED);
  });

  it("reports the path that configuration excludes as ignored, and its twin as an ordinary file", async () => {
    const files = await gitRail.projectFiles();

    expect(files).toContainEqual({ name: LISTED, ignored: false });
    expect(files).toContainEqual({ name: CONFIGURED_IGNORE, ignored: true });
  });
});
