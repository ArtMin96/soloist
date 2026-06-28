// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { addLocalCommand, addSharedCommand } from "@/api";
import { AddCommandModal } from "@/components/project-settings/AddCommandModal";
import type { ProcessSpec, Visibility } from "@/domain";

// Route the dialog's add through the real api wrappers so the test exercises the visibility split
// down to the invoked core command, not a stubbed callback.
const onAdd = (name: string, spec: ProcessSpec, visibility: Visibility) =>
  (visibility === "shared" ? addSharedCommand : addLocalCommand)(1, name, spec).then(() => {});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("AddCommandModal", () => {
  it("adds to the local overlay when 'Store locally only' is chosen", async () => {
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      return [];
    });

    render(<AddCommandModal open onOpenChange={() => {}} onAdd={onAdd} />);

    fireEvent.change(screen.getByLabelText("Command name"), { target: { value: "Worker" } });
    fireEvent.change(screen.getByLabelText("Command"), { target: { value: "npm run worker" } });
    fireEvent.click(screen.getByText("Store locally only"));
    fireEvent.click(screen.getByRole("button", { name: "Add command" }));

    await waitFor(() => expect(calls).toContain("add_local_command"));
    expect(calls).not.toContain("add_shared_command");
  });
});
