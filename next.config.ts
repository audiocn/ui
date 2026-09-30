import { createMDX } from "fumadocs-mdx/next";
import type { NextConfig } from "next";

const withMDX = createMDX();

const nextConfig: NextConfig = {
  outputFileTracingIncludes: {
    "/docs/**/*": [
      "./content/docs/**/*",
      "./components/**/*",
      "./hooks/**/*",
      "./lib/**/*",
    ],
  },
};

export default withMDX(nextConfig);
