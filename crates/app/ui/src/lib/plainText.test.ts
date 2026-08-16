import { describe, expect, it } from "vitest";
import { plainReason } from "@/lib/plainText";

describe("plainReason", () => {
  it("renders the requester's words unchanged when they are only words", () => {
    expect(plainReason("the release build needs it")).toBe("the release build needs it");
  });

  it("flattens control characters a reason could use to redraw the line", () => {
    // A carriage return can overwrite what was already drawn, and a bidirectional override can
    // reverse it — both let attacker-supplied text claim to say something it does not.
    expect(plainReason("safe\r‮evil")).toBe("safe  evil");
  });

  it("collapses padding that would push the command line out of view", () => {
    expect(plainReason("first\n\n\n\n\n\nsecond")).toBe("first\n\nsecond");
  });
});
