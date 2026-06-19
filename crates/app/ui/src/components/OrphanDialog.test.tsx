// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { OrphanDialog } from "@/components/OrphanDialog";
import type { OrphanInfo } from "@/domain";

const ORPHANS: OrphanInfo[] = [
  { name: "web", command: "npm run dev", pgid: 555 },
  { name: "worker", command: "node worker.js", pgid: 556 },
];

afterEach(cleanup);

describe("OrphanDialog", () => {
  it("stays closed when there are no orphans", () => {
    render(
      <OrphanDialog orphans={null} onKillOne={() => {}} onKillAll={() => {}} onLeave={() => {}} />,
    );
    expect(screen.queryByText("Leftover processes found")).toBeNull();
  });

  it("lists each leftover group with its command and pgid", () => {
    render(
      <OrphanDialog
        orphans={ORPHANS}
        onKillOne={() => {}}
        onKillAll={() => {}}
        onLeave={() => {}}
      />,
    );
    expect(screen.getByText("Leftover processes found")).toBeTruthy();
    expect(screen.getByText("web")).toBeTruthy();
    expect(screen.getByText("npm run dev")).toBeTruthy();
    expect(screen.getByText("pgid 556")).toBeTruthy();
  });

  it("routes each decision to its callback", () => {
    const onKillOne = vi.fn();
    const onKillAll = vi.fn();
    const onLeave = vi.fn();
    render(
      <OrphanDialog
        orphans={ORPHANS}
        onKillOne={onKillOne}
        onKillAll={onKillAll}
        onLeave={onLeave}
      />,
    );

    fireEvent.click(screen.getByLabelText("Kill web"));
    expect(onKillOne).toHaveBeenCalledWith(555);

    fireEvent.click(screen.getByText("Kill all"));
    expect(onKillAll).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Leave running"));
    expect(onLeave).toHaveBeenCalledTimes(1);
  });
});
