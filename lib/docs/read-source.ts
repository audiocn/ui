import { readFile } from "node:fs/promises";
import path from "node:path";

const TRAILING_NEWLINES = /\n+$/u;

export const readSource = async (relativePath: string): Promise<string> => {
  const absolutePath = path.join(process.cwd(), relativePath);
  const content = await readFile(absolutePath, "utf-8");
  return content.replace(TRAILING_NEWLINES, "");
};
