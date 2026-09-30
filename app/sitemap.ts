import type { MetadataRoute } from "next";

import { siteConfig } from "@/lib/site";
import { source } from "@/lib/source";

const sitemap = (): MetadataRoute.Sitemap => [
  { url: siteConfig.url },
  { url: new URL("/contributors", siteConfig.url).href },
  ...source.getPages().map((page) => ({
    url: new URL(page.url, siteConfig.url).href,
  })),
];

export default sitemap;
