// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// jsdom has no raster canvas, so the two browser behaviours `toRgb` is built on are stubbed here:
// assigning a value the context cannot parse leaves `fillStyle` untouched, and a painted pixel reads
// back as sRGB bytes. Both were confirmed against the real WebKitGTK webview the app ships on, where
// an `oklch()` value survives `fillStyle` verbatim — which is why reading `fillStyle` back is not
// enough and the colour has to be painted.

type Pixel = [number, number, number, number];

/** What each colour this stub knows how to parse paints. Anything else is rejected. */
const PAINTS: Record<string, Pixel> = {
  "#000000": [0, 0, 0, 255],
  "#000001": [0, 0, 1, 255],
  "#abcdef": [171, 205, 239, 255],
  "oklch(0.275 0.013 255)": [35, 40, 46, 255],
  "oklch(1 0 0 / 9%)": [255, 255, 255, 23],
};

/** Install a fake 2-D context, or `null` to stand in for a renderer with no raster canvas. */
function installCanvas({ hasContext }: { hasContext: boolean }): void {
  let fill = "#000000";
  let painted: Pixel = [0, 0, 0, 255];
  const context = {
    get fillStyle() {
      return fill;
    },
    set fillStyle(value: string) {
      if (value in PAINTS) fill = value;
    },
    clearRect: () => {},
    fillRect: () => {
      painted = PAINTS[fill];
    },
    getImageData: () => ({ data: painted }),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() =>
    hasContext ? (context as unknown as CanvasRenderingContext2D) : null,
  );
}

// Imported fresh per case so each gets its own lazily-created context rather than a cached one.
async function toRgb(raw: string): Promise<string> {
  const module = await import("./color");
  return module.toRgb(raw);
}

beforeEach(() => vi.resetModules());
afterEach(() => vi.restoreAllMocks());

describe("toRgb", () => {
  it("converts an oklch token to the sRGB Mermaid's colour engine can parse", async () => {
    installCanvas({ hasContext: true });

    expect(await toRgb("oklch(0.275 0.013 255)")).toBe("rgb(35, 40, 46)");
  });

  it("keeps a token's transparency instead of flattening it to opaque", async () => {
    installCanvas({ hasContext: true });

    expect(await toRgb("oklch(1 0 0 / 9%)")).toBe("rgba(255, 255, 255, 0.09)");
  });

  it("reports a legacy colour as its sRGB value", async () => {
    installCanvas({ hasContext: true });

    expect(await toRgb("#abcdef")).toBe("rgb(171, 205, 239)");
  });

  it("returns a colour the canvas rejects unchanged, rather than the seeded fallback", async () => {
    installCanvas({ hasContext: true });

    expect(await toRgb("not-a-colour")).toBe("not-a-colour");
  });

  it("returns the input unchanged where there is no raster canvas", async () => {
    installCanvas({ hasContext: false });

    expect(await toRgb("oklch(0.275 0.013 255)")).toBe("oklch(0.275 0.013 255)");
  });

  it("leaves a blank value alone", async () => {
    installCanvas({ hasContext: true });

    expect(await toRgb("")).toBe("");
  });
});
