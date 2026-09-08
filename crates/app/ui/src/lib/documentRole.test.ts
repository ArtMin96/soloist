import { describe, expect, it } from "vitest";
import type { DocumentParticipant } from "@/domain";
import { participantsLabel, strongestRole } from "@/lib/documentRole";

function participant(role: DocumentParticipant["role"], label = "lead"): DocumentParticipant {
  return { process: 1, label, role };
}

describe("strongestRole", () => {
  it("picks implementing over editing over reading from an unordered list", () => {
    expect(strongestRole([participant("reading"), participant("editing")])).toBe("editing");
    expect(
      strongestRole([participant("reading"), participant("editing"), participant("implementing")]),
    ).toBe("implementing");
  });

  it("returns the single role present when there is only one", () => {
    expect(strongestRole([participant("reading")])).toBe("reading");
  });
});

describe("participantsLabel", () => {
  it("names every participant with its role", () => {
    const label = participantsLabel([
      { process: 3, label: "lead", role: "implementing" },
      { process: 4, label: "worker-1", role: "reading" },
    ]);
    expect(label).toBe("lead — Implementing, worker-1 — Reading");
  });
});
