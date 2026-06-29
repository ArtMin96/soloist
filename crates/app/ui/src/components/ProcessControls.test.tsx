// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ProcessControls } from "@/components/ProcessControls";
import type { ProcStatus } from "@/domain";

const noop = () => {};

function renderControls(
  props: Partial<React.ComponentProps<typeof ProcessControls>> & { status: ProcStatus },
) {
  return render(<ProcessControls onStart={noop} onStop={noop} onRestart={noop} {...props} />);
}

afterEach(cleanup);

describe("ProcessControls resume affordance", () => {
  it("offers Resume beside Start for a stopped resumable agent", () => {
    renderControls({ status: "Stopped", resumable: true, onResume: noop });
    expect(screen.getByLabelText("Resume last session")).toBeTruthy();
    expect(screen.getByLabelText("Start")).toBeTruthy();
  });

  it("does not offer Resume for a non-resumable process", () => {
    renderControls({ status: "Stopped", resumable: false, onResume: noop });
    expect(screen.queryByLabelText("Resume last session")).toBeNull();
  });

  it("resumes the agent's last session when clicked", () => {
    const onResume = vi.fn();
    renderControls({ status: "Stopped", resumable: true, onResume });
    fireEvent.click(screen.getByLabelText("Resume last session"));
    expect(onResume).toHaveBeenCalledOnce();
  });

  it("disables Resume while the agent is running (resting-state action, no row reflow)", () => {
    renderControls({ status: "Running", resumable: true, onResume: noop });
    // The control stays present (so the cluster never reflows) but is disabled until it rests.
    expect(screen.getByLabelText("Resume last session")).toHaveProperty("disabled", true);
  });
});
