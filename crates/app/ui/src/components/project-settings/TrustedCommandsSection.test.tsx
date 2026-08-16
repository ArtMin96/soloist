// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@/api", () => ({
  trustGrants: vi.fn(),
  trustRevoke: vi.fn(),
}));

import { trustGrants, trustRevoke } from "@/api";
import { TrustedCommandsSection } from "@/components/project-settings/TrustedCommandsSection";
import type { TrustGrant } from "@/domain";

const list = vi.mocked(trustGrants);
const revoke = vi.mocked(trustRevoke);

const AUTHORED: TrustGrant = {
  variant_hash: "authored-v1",
  command: "npm run dev",
  requested_by: null,
  reason: null,
  granted_at_unix_millis: null,
};

const REQUESTED: TrustGrant = {
  variant_hash: "requested-v1",
  command: "npm run build",
  requested_by: 42,
  reason: "the release build needs it",
  granted_at_unix_millis: 1_700,
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("TrustedCommandsSection", () => {
  it("says a project trusts nothing rather than showing an empty frame", async () => {
    list.mockResolvedValue([]);
    render(<TrustedCommandsSection project={1} />);
    await waitFor(() =>
      expect(screen.getByText("Nothing is trusted in this project yet.")).toBeTruthy(),
    );
  });

  it("leads every grant with the command line it authorizes", async () => {
    list.mockResolvedValue([AUTHORED, REQUESTED]);
    render(<TrustedCommandsSection project={1} />);

    // The digest is the key, but a key is not something a person can review.
    await waitFor(() => expect(screen.getByText("npm run dev")).toBeTruthy());
    expect(screen.getByText("npm run build")).toBeTruthy();
    expect(screen.queryByText("authored-v1")).toBeNull();
  });

  it("tells a grant the user authored from one made at a process's asking", async () => {
    list.mockResolvedValue([AUTHORED, REQUESTED]);
    render(<TrustedCommandsSection project={1} />);

    await waitFor(() =>
      expect(screen.getByText("You approved this from this project.")).toBeTruthy(),
    );
    expect(
      screen.getByText(/Approved at the asking of process 42, which said: “the release build/),
    ).toBeTruthy();
  });

  it("revokes by the grant's own key and re-reads the list", async () => {
    list.mockResolvedValue([REQUESTED]);
    revoke.mockResolvedValue(undefined);
    render(<TrustedCommandsSection project={1} />);
    await waitFor(() => expect(screen.getByLabelText("Revoke npm run build")).toBeTruthy());

    list.mockResolvedValue([]);
    fireEvent.click(screen.getByLabelText("Revoke npm run build"));

    expect(revoke).toHaveBeenCalledWith(1, "requested-v1");
    await waitFor(() =>
      expect(screen.getByText("Nothing is trusted in this project yet.")).toBeTruthy(),
    );
  });

  it("keeps the grant listed when revoking failed", async () => {
    list.mockResolvedValue([REQUESTED]);
    revoke.mockRejectedValue(new Error("the store is unavailable"));
    render(<TrustedCommandsSection project={1} />);
    await waitFor(() => expect(screen.getByLabelText("Revoke npm run build")).toBeTruthy());

    fireEvent.click(screen.getByLabelText("Revoke npm run build"));

    await waitFor(() => expect(screen.getByText(/the store is unavailable/)).toBeTruthy());
    expect(screen.getByText("npm run build")).toBeTruthy();
  });
});
