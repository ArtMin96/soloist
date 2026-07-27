import { describe, expect, it } from "vitest";
import {
  activateProcess,
  forgetProcesses,
  mostRecentAvailableProcess,
} from "@/store/processActivationHistory";

describe("process activation history", () => {
  it("keeps unique process ids in most-recently-used order without a cap", () => {
    let history: number[] = [];
    for (let id = 1; id <= 8; id += 1) history = activateProcess(history, id);

    history = activateProcess(history, 3);

    expect(history).toEqual([3, 8, 7, 6, 5, 4, 2, 1]);
  });

  it("forgets lifecycle targets and resolves only a previously activated available process", () => {
    const history = forgetProcesses([4, 3, 2, 1], [4, 2]);

    expect(history).toEqual([3, 1]);
    expect(mostRecentAvailableProcess(history, [1, 2, 3])).toBe(3);
    expect(mostRecentAvailableProcess(history, [2, 4])).toBeNull();
  });
});
