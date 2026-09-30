import type { Metadata } from "next";

import { siteConfig } from "@/lib/site";

export const getSocialMetadata = (
  title: string,
  description: string = siteConfig.description
): Pick<Metadata, "openGraph" | "twitter"> => ({
  openGraph: {
    description,
    images: [
      {
        alt: "audiocn — audio components for React",
        height: 630,
        url: "/opengraph-image",
        width: 1200,
      },
    ],
    siteName: siteConfig.name,
    title,
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    description,
    images: ["/opengraph-image"],
    title,
  },
});
