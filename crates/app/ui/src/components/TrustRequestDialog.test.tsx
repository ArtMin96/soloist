// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TrustRequestDialog } from "@/components/TrustRequestDialog";
import type { TrustRequest } from "@/domain";

const REQUEST: TrustRequest = {
  id: 7,
  project: 1,
  requested_by: 42,
  requested_by_label: "lead",
  review: {
    name: "build",
    variant_hash: "variant-v1",
    command: "npm run build",
    working_dir: "web",
    env: { CI: "1" },
  },
  reason: "the release build needs it",
  expires_unix_millis: 0,
};

function open(overrides: Partial<Parameters<typeof TrustRequestDialog>[0]> = {}) {
  const onApprove = vi.fn();
  const onDeny = vi.fn();
  render(
    <TrustRequestDialog
      requests={[REQUEST]}
      onApprove={onApprove}
      onDeny={onDeny}
      {...overrides}
    />,
  );
  return { onApprove, onDeny };
}

afterEach(cleanup);

describe("TrustRequestDialog", () => {
  it("shows nothing when nothing is being asked", () => {
    open({ requests: [] });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows what would run, where, and with what environment", () => {
    open();
    expect(screen.getByText("npm run build")).toBeTruthy();
    expect(screen.getByText("in web")).toBeTruthy();
    expect(screen.getByText("CI=1")).toBeTruthy();
  });

  it("names and numbers the process that is asking", () => {
    open();
    // A label alone is chosen by the same side that wrote the reason; the id is the part the
    // user can check against the process list.
    expect(screen.getByText(/lead \(process 42\), in its own words/)).toBeTruthy();
  });

  it("quotes the reason as the requester's words rather than the app's", () => {
    open();
    const quotation = screen.getByText("the release build needs it");
    expect(quotation.tagName).toBe("BLOCKQUOTE");
  });

  it("leaves approve unfocused and focuses deny instead", () => {
    open();
    // Approval fatigue is the failure this dialog is built against: a focused Approve turns a
    // reflexive Enter into arbitrary code execution.
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Deny" }));
  });

  it("treats dismissing the prompt as denying it", () => {
    const { onApprove, onDeny } = open();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onDeny).toHaveBeenCalledWith(REQUEST);
    expect(onApprove).not.toHaveBeenCalled();
  });

  it("routes each decision to its callback", () => {
    const { onApprove, onDeny } = open();
    fireEvent.click(screen.getByRole("button", { name: "Approve" }));
    expect(onApprove).toHaveBeenCalledWith(REQUEST);
    fireEvent.click(screen.getByRole("button", { name: "Deny" }));
    expect(onDeny).toHaveBeenCalledWith(REQUEST);
  });

  it("decides one request at a time and says how many are queued", () => {
    const second = { ...REQUEST, id: 8, review: { ...REQUEST.review, command: "rm -rf /" } };
    open({ requests: [REQUEST, second] });
    expect(screen.getByText("npm run build")).toBeTruthy();
    expect(screen.queryByText("rm -rf /")).toBeNull();
    expect(screen.getByText("1 more waiting")).toBeTruthy();
  });
});
