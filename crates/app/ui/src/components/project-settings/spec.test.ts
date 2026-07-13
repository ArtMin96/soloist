import { describe, expect, it } from "vitest";
import { buildSpec, specOf } from "@/components/project-settings/spec";
import type { ProjectCommandView } from "@/domain";

// A settings-page view of one shared command, overridable per test.
const view = (over: Partial<ProjectCommandView> = {}): ProjectCommandView => ({
  name: "Web",
  command: "npm run dev",
  working_dir: null,
  auto_start: true,
  auto_restart: false,
  restart_when_changed: [],
  visibility: "shared",
  terminal_alerts: true,
  status: null,
  env: {},
  ...over,
});

describe("specOf", () => {
  it("preserves the command's env so an edit cannot wipe a committed env block", () => {
    expect(specOf(view({ env: { A: "1", B: "2" } })).env).toEqual({ A: "1", B: "2" });
  });

  it("keeps env when the editor patches one field on top of the current spec", () => {
    // The editor persists `{ ...specOf(command), ...patch }`; the patch never touches env.
    const patched = { ...specOf(view({ env: { A: "1" } })), auto_start: false };
    expect(patched.env).toEqual({ A: "1" });
    expect(patched.auto_start).toBe(false);
  });

  it("carries no env for a command that has none", () => {
    expect(specOf(view()).env).toEqual({});
  });
});

describe("buildSpec", () => {
  it("round-trips a non-empty env verbatim", () => {
    expect(
      buildSpec({
        command: "npm run dev",
        working_dir: null,
        auto_start: true,
        auto_restart: false,
        restart_when_changed: [],
        env: { PORT: "3000" },
      }).env,
    ).toEqual({ PORT: "3000" });
  });
});
