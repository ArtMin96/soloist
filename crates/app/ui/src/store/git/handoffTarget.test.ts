import { describe, expect, it } from "vitest";
import { handoffTarget } from "@/store/git/handoffTarget";
import type { ProcessView } from "@/domain";

const PROJECT = 7;

function process(over: Partial<ProcessView> = {}): ProcessView {
  return {
    id: 3,
    project: PROJECT,
    kind: "Agent",
    label: "worker",
    status: "Running",
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
    ...over,
  };
}

describe("handoffTarget", () => {
  it("names the running agent the reader is looking at", () => {
    expect(handoffTarget(process(), PROJECT)).toBe(3);
  });

  it("names nothing for a process in another project, so the core is never asked to reach one", () => {
    expect(handoffTarget(process({ project: 99 }), PROJECT)).toBeNull();
  });

  it("names nothing for a terminal or a command, which have no session to hand work to", () => {
    expect(handoffTarget(process({ kind: "Terminal" }), PROJECT)).toBeNull();
    expect(handoffTarget(process({ kind: "Command" }), PROJECT)).toBeNull();
  });

  it("names nothing for an agent that is not running, since there is nothing listening", () => {
    expect(handoffTarget(process({ status: "Stopped" }), PROJECT)).toBeNull();
  });

  it("names nothing when nothing is selected, leaving the choice to the core", () => {
    expect(handoffTarget(null, PROJECT)).toBeNull();
  });
});
