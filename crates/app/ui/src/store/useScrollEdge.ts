import { useEffect, useRef, useState } from "react";

// Tracks whether a scroll container has moved away from its top, so a header can show the
// macOS "scroll-edge" hairline only once content slides under it (borderless at rest). Returns
// a ref for the scroll element and the live `scrolled` flag. Reuse on any scroll-under-header
// surface (settings, project settings, orchestration) so the cue stays consistent.
export function useScrollEdge<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const onScroll = () => setScrolled(el.scrollTop > 0);
    onScroll();
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  return { ref, scrolled };
}
