import type { MetadataRoute } from "next";

import { siteConfig } from "@/lib/site";

const robots = (): MetadataRoute.Robots => ({
  rules: { allow: "/", disallow: "/api/", userAgent: "*" },
  sitemap: new URL("/sitemap.xml", siteConfig.url).href,
});

export default robots;
