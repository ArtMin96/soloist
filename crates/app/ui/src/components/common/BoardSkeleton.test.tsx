// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { BoardSkeleton } from "@/components/common/BoardSkeleton";
import { Skeleton } from "@/components/ui/skeleton";

afterEach(cleanup);

describe("BoardSkeleton", () => {
  it("reserves trailing space for the result tally and primary action", () => {
    const { container } = render(
      <BoardSkeleton facets={<Skeleton className="h-7 w-28" />} rows={0} row={() => null} />,
    );
    const standIns = container.querySelectorAll('[data-slot="skeleton"]');

    expect(standIns).toHaveLength(4);
    expect(standIns[2]?.className).toContain("h-3.5 w-12");
    expect(standIns[3]?.className).toContain("h-7 w-24");
  });
});
