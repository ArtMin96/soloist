import type { AgentActivity, ProcStatus, ProjectView } from "@domain";
import { closeLeadFromOutside, LEAD_AGENT } from "../../src/harness/leadAgent.js";
import { openProject } from "../../src/flows/openProject.js";
import { launchAgent } from "../../src/flows/launchAgent.js";
import { orchestrationPane } from "../../src/screens/OrchestrationPane.js";
import { sidebar } from "../../src/screens/Sidebar.js";

// The lead the walk launches manually: its stub binds its own MCP session and `spawn_agent`s a
// worker over the real IPC path, so the worker's lineage is recorded under it. The worker is a real
// spawned agent whose visible-output idle heuristic drives a deterministic activity flip.
const LEAD = LEAD_AGENT.lead;
const WORKER = LEAD_AGENT.worker;
// A second, independent manual launch — the existing sleep-`claude` stub — with no lineage: it must
// appear as a root, the contrast that proves nesting reflects a real lead→worker edge, not a coincidence.
const MANUAL = "Claude";

const RUNNING: ProcStatus = "Running";
const WORKING: AgentActivity = "Working";
const IDLE: AgentActivity = "Idle";

// The agent lineage tree, driven in the real window against the real core: a bound lead's spawned
// worker nests under it; a manual launch is a root; a worker's glyph flips on a real activity event;
// a closed lead re-roots its workers. Every assertion keys on tree structure or live state the core
// produced — a repaint or a dropped event cannot fake a recorded parent→child edge or a real
// idle-FSM transition.
describe("the agent lineage tree", () => {
  let project: ProjectView;

  before(async () => {
    project = await openProject("orchestration");

    // The lead: a manual launch whose stub then spawns a worker into this project over MCP.
    await launchAgent(LEAD);
    await sidebar.waitForRowStatus(LEAD, RUNNING);

    // A second, unrelated manual agent — a root with no children.
    await launchAgent(MANUAL);
    await sidebar.waitForRowStatus(MANUAL, RUNNING);

    await sidebar.openOrchestration(project.name);
    await orchestrationPane.waitForTree();
  });

  after(async () => {
    // Leave nothing running: the lead may already be gone (the re-root test closes it), the worker
    // survives that close as a root, and the manual agent is still up. `stopIfRunning` tolerates a
    // missing row, so a failed spec's real failure is never masked by cleanup.
    await sidebar.stopIfRunning(LEAD);
    await sidebar.stopIfRunning(WORKER);
    await sidebar.stopIfRunning(MANUAL);
  });

  it("nests a spawned worker under the lead that spawned it", async () => {
    const [lead] = await orchestrationPane.waitForNodes(LEAD);
    const [worker] = await orchestrationPane.waitForNodes(WORKER);

    // The worker nests under the lead: the tree resolves its parent to the lead's own node, one
    // level deeper. That edge exists only because the core recorded lineage on the bound-lead spawn.
    expect(worker.parent).toBe(lead.id);
    expect(worker.level).toBe(lead.level + 1);
    // The lead itself, a manual launch, is a root.
    expect(lead.parent).toBeNull();
  });

  it("shows a manual launch as a root with no children", async () => {
    const [manual] = await orchestrationPane.waitForNodes(MANUAL);

    expect(manual.parent).toBeNull();
    expect(manual.level).toBe(1);
    // No node nests under it — a manual launch spawned nothing.
    const nodes = await orchestrationPane.nodes();
    expect(nodes.some((node) => node.parent === manual.id)).toBe(false);
  });

  it("flips a worker's activity glyph on a real transition", async () => {
    // The worker stub cycles output then quiet, so the real idle sampler classifies it Working, then
    // Idle. Observing both — on the same node, without a refresh — proves the glyph follows a real
    // activity transition end to end, not a static paint.
    await orchestrationPane.waitForActivity(WORKER, WORKING);
    await orchestrationPane.waitForActivity(WORKER, IDLE);
  });

  it("re-roots a closed lead's workers", async () => {
    const [lead] = await orchestrationPane.waitForNodes(LEAD);
    const [worker] = await orchestrationPane.waitForNodes(WORKER);
    expect(worker.parent).toBe(lead.id);

    // Close the lead from outside the window — the only surface that removes a single agent. The
    // worker survives (its own process group) and re-roots, because the core drops a parent that
    // has left the registry.
    await closeLeadFromOutside();

    const rerooted = await orchestrationPane.waitForParent(WORKER, null);
    expect(rerooted.level).toBe(1);
    // The lead's node is gone, and its worker is neither stranded nor hidden.
    await orchestrationPane.waitForGone(LEAD);
  });
});
