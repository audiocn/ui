import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { socialCards } from "@/lib/social-catalog";
import socialImages from "@/lib/social-images.json";
import type { SocialImage } from "@/lib/social-metadata";

const images: Record<string, SocialImage | undefined> = socialImages;

describe("social image assets", () => {
  it("has unique capture IDs and page URLs", () => {
    expect(new Set(socialCards.map((card) => card.id)).size).toBe(
      socialCards.length
    );
    expect(new Set(socialCards.map((card) => card.pathname)).size).toBe(
      socialCards.length
    );
  });

  for (const card of socialCards) {
    it(`ships a valid, versioned image for ${card.pathname}`, () => {
      const image = images[card.pathname];
      expect(image).toBeDefined();
      if (!image) {
        return;
      }
      expect(image.alt).toBe(card.alt);
      const bytes = readFileSync(
        path.join(process.cwd(), "public", image.url.slice(1))
      );
      expect(bytes.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
      expect([bytes.readUInt32BE(16), bytes.readUInt32BE(20)]).toEqual([
        1200, 630,
      ]);
      expect(bytes.length).toBeLessThanOrEqual(1_500_000);
      const hash = createHash("sha256")
        .update(bytes)
        .digest("hex")
        .slice(0, 12);
      expect(image.url).toBe(`/og/${card.id}-${hash}.png`);
    });
  }
});
