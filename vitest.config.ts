import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": import.meta.dirname,
    },
  },
  test: {
    environment: "jsdom",
    exclude: ["node_modules", ".next", ".source", "e2e", "fixtures"],
    include: ["**/*.test.{ts,tsx}"],
    setupFiles: ["./test/setup.ts"],
  },
});
