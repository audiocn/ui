import { defineConfig } from "oxlint";
import core from "ultracite/oxlint/core";
import next from "ultracite/oxlint/next";
import react from "ultracite/oxlint/react";
import shadcn from "ultracite/oxlint/shadcn";

/**
 * Stock shadcn/ui components added by the CLI. They stay as upstream writes
 * them, so `shadcn add --diff` shows only real changes.
 */
const SHADCN_FILES = [
  "components/ui/alert.tsx",
  "components/ui/badge.tsx",
  "components/ui/card.tsx",
  "components/ui/context-menu.tsx",
  "components/ui/empty.tsx",
  "components/ui/field.tsx",
  "components/ui/input.tsx",
  "components/ui/kbd.tsx",
  "components/ui/label.tsx",
  "components/ui/popover.tsx",
  "components/ui/scroll-area.tsx",
  "components/ui/select.tsx",
  "components/ui/separator.tsx",
  "components/ui/skeleton.tsx",
  "components/ui/switch.tsx",
  "components/ui/tabs.tsx",
  "components/ui/toggle.tsx",
  "components/ui/tooltip.tsx",
];

export default defineConfig({
  extends: [core, react, next, shadcn],
  ignorePatterns: [
    ...(core.ignorePatterns ?? []),
    ".agents/**",
    ".claude/**",
    ".source/**",
    "public/r/**",
  ],
  jsPlugins: shadcn.jsPlugins,
  overrides: [
    {
      files: SHADCN_FILES,
      rules: {
        "eslint/eqeqeq": "off",
        "eslint/func-style": "off",
        "eslint/no-use-before-define": "off",
        "jsx-a11y/label-has-associated-control": "off",
        "react/function-component-definition": "off",
        "shadcn/no-inline-styles": "off",
        "shadcn/no-restyle": "off",
      },
    },
    {
      // Registry components: custom-drawn widgets (meters, knobs, canvases)
      // expose ARIA roles because no native element fits, composite widgets
      // handle keys on their container, and the code must not depend on
      // Next.js because it installs into any React app.
      files: ["components/ui/**"],
      rules: {
        "jsx-a11y/no-noninteractive-element-interactions": "off",
        "jsx-a11y/prefer-tag-over-role": "off",
        "nextjs/no-img-element": "off",
      },
    },
    {
      // App code using audiocn components: visualizers take their colour from
      // the text colour, and components are tuned through CSS variables, so
      // colour and variable classes are part of their contract.
      files: [
        "components/examples/**",
        "components/blocks/**",
        "components/docs/**",
        "app/**",
      ],
      rules: {
        "shadcn/no-restyle": [
          "error",
          {
            allow: ["layout"],
            contracts: [
              {
                allow: ["layout", "color", "typography", "effects", "[--*"],
                pattern:
                  "^(BarVisualizer|LiveWaveform|Spectrum|Waveform|DbReadout|DbScale|ClipIndicator|LevelMeter.*|Knob.*|Fader.*)$",
              },
              {
                allow: ["layout", "spacing", "shape", "color"],
                pattern:
                  "^(AudioPlayer|ChannelStrip|Mixer|TrackList|SoundPad|SoundPadGrid)$",
              },
            ],
          },
        ],
        "shadcn/no-unknown-classes": ["error", { allow: ["not-prose"] }],
      },
    },
  ],
});
