import { defineConfig } from "oxfmt";
import ultracite from "ultracite/oxfmt";

export default defineConfig({
  ...ultracite,
  ignorePatterns: [
    ...(ultracite.ignorePatterns ?? []),
    ".agents/**",
    ".claude/**",
    ".source/**",
    "public/r/**",
    "test-results/**",
    "playwright-report/**",
  ],
});
