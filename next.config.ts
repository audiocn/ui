import { createMDX } from "fumadocs-mdx/next";
import type { NextConfig } from "next";

const withMDX = createMDX();

const nextConfig: NextConfig = {
  devIndicators: process.env.AUDIOCN_SOCIAL_CAPTURE === "1" ? false : undefined,
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
