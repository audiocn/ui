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
  // Serves the Markdown twin of a docs page at `<page>.md`, for AI agents.
  rewrites: () =>
    Promise.resolve([
      { destination: "/llms.mdx/:path*", source: "/docs/:path*.md" },
    ]),
};

export default withMDX(nextConfig);
