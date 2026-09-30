/* oxlint-disable shadcn/no-inline-styles -- ImageResponse does not load site CSS; its renderer requires inline styles. */

import { ImageResponse } from "next/og";

export const alt = "audiocn — audio components for React, built the shadcn way";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const BAR_HEIGHTS = [24, 52, 78, 110, 156, 210, 260, 190, 132, 90, 58, 30];

const SocialImage = () =>
  new ImageResponse(
    <div
      style={{
        alignItems: "center",
        background: "#1c1917",
        color: "#fafaf9",
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
        <div style={{ color: "#d6d3d1", fontSize: 24 }}>
          Meters. Faders. Mixers. Players.
        </div>
      </div>
      <div style={{ alignItems: "center", display: "flex", gap: 9 }}>
        {BAR_HEIGHTS.map((height) => (
          <div
            key={height}
            style={{
              background: "#34d399",
              borderRadius: 8,
              height,
              width: 14,
            }}
          />
        ))}
      </div>
    </div>,
    size
  );

export default SocialImage;
