import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";

const dir = path.dirname(fileURLToPath(import.meta.url));
const proofDir = path.resolve(dir, "../../logs/proof");

/** Captures the real window and the terminal evidence that made a completed walk pass. */
export async function captureProof(
  name: string,
  evidence: Record<string, string | number | boolean | null>,
): Promise<void> {
  mkdirSync(proofDir, { recursive: true });
  await browser.saveScreenshot(path.join(proofDir, `${name}.png`));
  writeFileSync(
    path.join(proofDir, `${name}.json`),
    `${JSON.stringify(evidence, null, 2)}\n`,
  );
}
