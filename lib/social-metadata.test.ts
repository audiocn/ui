import { describe, expect, it } from "vitest";

import { siteConfig } from "@/lib/site";
import { getPageMetadata } from "@/lib/social-metadata";

describe("page metadata", () => {
  it("uses the homepage's descriptive title without adding a second brand", () => {
    const metadata = getPageMetadata({ pathname: "/", title: "audiocn" });
    expect(metadata.title).toEqual({ absolute: siteConfig.title });
    expect(metadata.openGraph?.title).toBe(siteConfig.title);
    expect(metadata.twitter?.title).toBe(siteConfig.title);
  });

  it("gives a nested page its own absolute canonical and social URL", () => {
    const metadata = getPageMetadata({
      description: "A stereo level meter.",
      pathname: "/docs/components/level-meter",
      title: "Level Meter for React",
    });
    const canonical = "https://audiocn.dev/docs/components/level-meter";
    expect(metadata.alternates?.canonical).toBe(canonical);
    expect(metadata.openGraph?.url).toBe(canonical);
    expect(metadata.title).toEqual({
      absolute: "Level Meter for React — audiocn",
    });
    expect(metadata.description).toBe("A stereo level meter.");
    expect(metadata.openGraph?.description).toBe(metadata.description);
    expect(metadata.twitter?.description).toBe(metadata.description);
  });

  it("shares the requested image and its accessible description across networks", () => {
    const metadata = getPageMetadata({
      image: {
        alt: "Two stereo meters with peak hold.",
        url: "/og/meters.png",
      },
      pathname: "/docs/components/level-meter",
      title: "Level Meter for React",
    });
    expect(metadata.openGraph?.images).toEqual([
      {
        alt: "Two stereo meters with peak hold.",
        height: 630,
        url: "https://audiocn.dev/og/meters.png",
        width: 1200,
      },
    ]);
    expect(metadata.twitter?.images).toEqual([
      {
        alt: "Two stereo meters with peak hold.",
        url: "https://audiocn.dev/og/meters.png",
      },
    ]);
  });
});
