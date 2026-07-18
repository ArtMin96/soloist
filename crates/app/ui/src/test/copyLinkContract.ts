import { renderHook, waitFor } from "@testing-library/react";
import { expect, vi, type Mock } from "vitest";

/**
 * Shared assertion for a store hook's "Copy link" action: invoking it writes to the clipboard
 * exactly the `solo://` link the core built for the target — never a link the window assembled. The
 * scratchpad and to-do editors expose the same `copyLink(id)` shape, so both cover the wiring the
 * same way. The real-window clipboard hop is unverifiable under WebKitGTK/WebDriver, so this proves
 * it headlessly; the link's own construction is proven in the core (`coordination/link` +
 * `facade/link` tests).
 */
export async function expectCopyLinkWritesCoreLink(opts: {
  useStore: (project: number) => { copyLink: (id: number) => void };
  linkFn: Mock;
  project: number;
  target: number;
  link: string;
}): Promise<void> {
  const { useStore, linkFn, project, target, link } = opts;
  linkFn.mockResolvedValue(link);
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.assign(navigator, { clipboard: { writeText } });

  const { result } = renderHook(() => useStore(project));
  result.current.copyLink(target);

  await waitFor(() => expect(writeText).toHaveBeenCalledWith(link));
  expect(linkFn).toHaveBeenCalledWith(project, target);
}
