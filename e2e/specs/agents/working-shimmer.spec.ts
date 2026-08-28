import type { ProcStatus } from "@domain";
import { launchAgent } from "../../src/flows/launch.js";
import { openProject } from "../../src/flows/openProject.js";
import { sidebar } from "../../src/screens/Sidebar.js";

// The agent the walk drives. Its fixture stub cycles a burst of output and a quiet spell, so the
// real idle sampler classifies it Working on its own and the name sweeps without the walk having to
// arrange anything about the sweep itself.
const AGENT = "OpenCode";
const RUNNING: ProcStatus = "Running";

// Sub-pixel layout rounding, nothing more: the highlight's box and the word's are laid out from the
// same text, so they either agree or disagree by the width of a name.
const SWEEP_TOLERANCE_PX = 1;

// A working agent reports progress in its name as well as its glyph: a highlight travels across the
// label. Whether it *travels* is geometry — the mask that reveals it is sized in percentages of the
// box it paints, so a box wider than the word spreads the band over the whole row and lights the
// name all at once instead of sweeping through it. Only a real window can answer that: jsdom
// measures every box as zero, so the component's own tests stayed green throughout exactly this
// defect and can never catch it.
describe("the name of a working agent", () => {
  before(async () => {
    await openProject("basic");
    await launchAgent(AGENT);
    await sidebar.waitForRowStatus(AGENT, RUNNING);
  });

  after(async () => {
    // Leave nothing running: an agent that outlives its app session is a leftover the next
    // session's app would (rightly) raise its orphan dialog over.
    await sidebar.stopIfRunning(AGENT);
  });

  it("sweeps a highlight across the word, not across the whole row", async () => {
    const { overlay, ink, cell } = await sidebar.waitForSweep(AGENT);

    // The highlight may be narrower than the name — that is a name too long for the row it is in,
    // clipped to what the row can show — but never wider than the name it is meant to travel
    // across.
    const overrun = overlay - Math.min(ink, cell);
    expect(overrun).toBeLessThanOrEqual(SWEEP_TOLERANCE_PX);
    expect(overrun).toBeGreaterThanOrEqual(-SWEEP_TOLERANCE_PX);
  });
});
