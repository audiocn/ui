import { readFile } from "node:fs/promises";
import path from "node:path";

const TRAILING_NEWLINES = /\n+$/u;

export const readSource = async (relativePath: string): Promise<string> => {
  const absolutePath = path.join(
    // Turbopack reads this hint and skips tracing the whole project.
    // oxlint-disable-next-line no-inline-comments
    /* turbopackIgnore: true */ process.cwd(),
    relativePath
  );
  const content = await readFile(absolutePath, "utf-8");
  return content.replace(TRAILING_NEWLINES, "");
};
