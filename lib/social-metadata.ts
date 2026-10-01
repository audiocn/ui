import type { Metadata } from "next";

import { siteConfig } from "@/lib/site";

export interface SocialImage {
  alt: string;
  url: string;
}

interface PageMetadataOptions {
  title: string;
  pathname: string;
  description?: string;
  image?: SocialImage;
}

const defaultImage: SocialImage = {
  alt: "audiocn — audio components for React, built the shadcn way",
  url: "/opengraph-image",
};

export const getPageMetadata = ({
  title,
  pathname,
  description = siteConfig.description,
  image = defaultImage,
}: PageMetadataOptions): Metadata => {
  const fullTitle =
    pathname === "/" ? siteConfig.title : `${title} — ${siteConfig.name}`;
  const url =
    pathname === "/" ? siteConfig.url : new URL(pathname, siteConfig.url).href;
  const imageUrl = new URL(image.url, siteConfig.url).href;

  return {
    alternates: { canonical: url },
    description,
    openGraph: {
      description,
      images: [{ alt: image.alt, height: 630, url: imageUrl, width: 1200 }],
      locale: "en_US",
      siteName: siteConfig.name,
      title: fullTitle,
      type: "website",
      url,
    },
    title: { absolute: fullTitle },
    twitter: {
      card: "summary_large_image",
      description,
      images: [{ alt: image.alt, url: imageUrl }],
      title: fullTitle,
    },
  };
};
