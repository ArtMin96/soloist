import { useCallback, useEffect, useRef, useState } from "react";
import { templateRender } from "@/api";
import { RENDERABLE_TEMPLATE_KIND, templateScopeProject } from "@/lib/templates";
import type { RenderedPrompt, TemplateKind, TemplateScope } from "@/domain";

// The quiet window after the last keystroke before the preview re-renders. Each render is an IPC
// round-trip that re-reads the stored body, so typing a value must not send one per character.
// Short enough to read as live; named so a future settings knob can promote it without touching
// the call site.
const RENDER_DEBOUNCE_MS = 200;

/** Which template the preview renders, and the revision that re-reads it when the stored body moves. */
export interface TemplateRenderTarget {
  kind: TemplateKind | null;
  scope: TemplateScope | null;
  name: string | null;
  /** The project whose library the template was opened from; the global library has none. */
  project: number | null;
  /** The revision the template is loaded at. A new one means the body changed, so re-render. */
  revision: number | null;
}

export interface TemplateRenderStore {
  /** Whether this template's kind is rendered at all — false leaves the surface with nothing to show. */
  renderable: boolean;
  /** The value typed for each placeholder name. A name absent here has no value supplied. */
  values: Record<string, string>;
  setValue: (placeholder: string, value: string) => void;
  /** The latest render, or null before the first one resolves (or when there is nothing to render). */
  rendered: RenderedPrompt | null;
  /** A refused render, surfaced verbatim from the core, or null. */
  error: string | null;
}

// Drives the template manager's live preview: the value the user typed per placeholder, and the
// prompt the core renders from them. Substitution never happens here — every preview is one
// `template_render` call, so the text on screen is produced by the same core command an agent reaches
// over MCP and cannot drift from it.
//
// The render reads the *stored* body, so it is keyed to the loaded revision rather than to whatever
// sits in the editor: an unsaved edit appears once autosave lands, which is also when the declared
// placeholder list moves. Only a prompt template renders (the seedable kinds' markers are content the
// author goes on to edit), so every other kind reports `renderable: false` and never calls the core.
export function useTemplateRender({
  kind,
  scope,
  name,
  project,
  revision,
}: TemplateRenderTarget): TemplateRenderStore {
  const [values, setValues] = useState<Record<string, string>>({});
  const [rendered, setRendered] = useState<RenderedPrompt | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Renders resolve out of order (a fast small one can overtake a slow large one), so each is stamped
  // and only the newest lands. Without this the pane can settle on the result of a keystroke the user
  // has already typed past.
  const latest = useRef(0);

  const renderable = kind === RENDERABLE_TEMPLATE_KIND;
  const target = `${kind}:${scope}:${name}`;

  // A newly opened template starts with no values — the previous one's fill-ins mean nothing here.
  // Adjusted during render rather than in an effect so the render below never fires once with the
  // outgoing template's values against the incoming template's name.
  const [openTarget, setOpenTarget] = useState(target);
  if (openTarget !== target) {
    setOpenTarget(target);
    setValues({});
    setRendered(null);
    setError(null);
  }

  useEffect(() => {
    if (!renderable || name == null || revision == null) return;
    // Debounced through the effect's own cleanup: any further keystroke cancels this pending render
    // before it is sent, so a burst of typing costs one round-trip rather than one per character.
    const timer = setTimeout(() => {
      const stamp = ++latest.current;
      templateRender(templateScopeProject(scope, project), name, values).then(
        (result) => {
          if (stamp !== latest.current) return;
          setRendered(result);
          setError(null);
        },
        (reason) => {
          if (stamp !== latest.current) return;
          setRendered(null);
          setError(String(reason));
        },
      );
    }, RENDER_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [renderable, scope, project, name, revision, values]);

  // An empty field is not a supplied value, so it is dropped from the map rather than sent as "".
  // The core reads a present key as answered, so sending "" would substitute the marker away and
  // leave `review ` — a sentence that reads as complete with its subject silently missing, and no
  // longer reported as unfilled. Clearing a field must put the marker back, not hide it.
  const setValue = useCallback((placeholder: string, value: string) => {
    setValues((prev) => {
      const next = { ...prev };
      if (value === "") delete next[placeholder];
      else next[placeholder] = value;
      return next;
    });
  }, []);

  return { renderable, values, setValue, rendered, error };
}
