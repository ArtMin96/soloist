import type { ThemeColorRole, ThemeColors } from "@/domain";

const COLOR_PROPERTIES = [
  "color",
  "backgroundColor",
  "borderTopColor",
  "borderRightColor",
  "borderBottomColor",
  "borderLeftColor",
  "outlineColor",
  "textDecorationColor",
  "caretColor",
  "accentColor",
  "fill",
  "stroke",
  "boxShadow",
] as const;

function comparableColor(value: string): string {
  const probe = document.createElement("span");
  probe.style.color = value;
  document.body.append(probe);
  const result = getComputedStyle(probe).color;
  probe.remove();
  return result;
}

function elementUsesColor(element: Element, color: string): boolean {
  const computed = getComputedStyle(element);
  return COLOR_PROPERTIES.some(
    (property) => computed[property] === color || computed[property].includes(color),
  );
}

function mark(elements: Element[]): () => void {
  const originals = elements.map((element) => {
    const html = element as HTMLElement;
    const original = { outline: html.style.outline, outlineOffset: html.style.outlineOffset };
    html.style.outline = "2px solid var(--ring)";
    html.style.outlineOffset = "2px";
    return { html, original };
  });
  return () => {
    for (const { html, original } of originals) {
      html.style.outline = original.outline;
      html.style.outlineOffset = original.outlineOffset;
    }
  };
}

export function highlightThemeRoleUsage(color: string): () => void {
  const comparable = comparableColor(color);
  const elements = [...document.body.querySelectorAll("*")].filter(
    (element) => !element.closest("[data-theme-editor]") && elementUsesColor(element, comparable),
  );
  return mark(elements);
}

export function themeRoleAtElement(element: Element, colors: ThemeColors): ThemeColorRole | null {
  const computed = getComputedStyle(element);
  const used = new Set(COLOR_PROPERTIES.map((property) => computed[property]));
  return (
    (Object.entries(colors).find(([, value]) => used.has(comparableColor(value)))?.[0] as
      | ThemeColorRole
      | undefined) ?? null
  );
}

export function startThemeInspector(
  colors: ThemeColors,
  onPick: (role: ThemeColorRole) => void,
): () => void {
  let clearMark = () => {};
  const move = (event: PointerEvent) => {
    clearMark();
    const target = event.target;
    if (target instanceof Element && !target.closest("[data-theme-editor]")) {
      clearMark = mark([target]);
    }
  };
  const click = (event: MouseEvent) => {
    const target = event.target;
    if (!(target instanceof Element) || target.closest("[data-theme-editor]")) return;
    const role = themeRoleAtElement(target, colors);
    if (!role) return;
    event.preventDefault();
    event.stopPropagation();
    onPick(role);
  };
  document.addEventListener("pointermove", move, true);
  document.addEventListener("click", click, true);
  return () => {
    clearMark();
    document.removeEventListener("pointermove", move, true);
    document.removeEventListener("click", click, true);
  };
}
