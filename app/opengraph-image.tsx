/* oxlint-disable shadcn/no-inline-styles -- ImageResponse does not load site CSS; its renderer requires inline styles. */
/* oxlint-disable nextjs/no-img-element -- ImageResponse renders embedded image bytes, not Next.js image optimization. */

import { readFile } from "node:fs/promises";
import path from "node:path";

import { ImageResponse } from "next/og";

export const alt = "audiocn — audio components for React, built the shadcn way";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const SocialImage = async () => {
  const logo = await readFile(
    path.join(process.cwd(), "public/brand/logo.png")
  );
  return new ImageResponse(
    <div
      style={{
        alignItems: "center",
        background: "#fafaf9",
        color: "#1c1917",
        display: "flex",
        height: "100%",
        justifyContent: "space-between",
        padding: 80,
        width: "100%",
      }}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 30,
          maxWidth: 630,
        }}
      >
        <div style={{ fontSize: 80, fontWeight: 700, letterSpacing: -4 }}>
          audiocn
        </div>
        <div style={{ fontSize: 42, lineHeight: 1.2 }}>
          Audio components for React, built the shadcn way.
        </div>
        <div style={{ color: "#57534e", fontSize: 24 }}>
          Meters. Faders. Mixers. Players.
        </div>
      </div>
      <img
        alt="audiocn knob logo"
        height={320}
        src={`data:image/png;base64,${logo.toString("base64")}`}
        width={320}
      />
    </div>,
    size
  );
};

export default SocialImage;
