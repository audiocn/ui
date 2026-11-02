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
  "components/ui/dropdown-menu.tsx",
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
  "components/ui/sonner.tsx",
  "components/ui/switch.tsx",
  "components/ui/tabs.tsx",
  "components/ui/toggle.tsx",
  "components/ui/tooltip.tsx",
];

/**
 * Code Block Command and its dependencies, added from chanhdai.com's registry.
 * They stay as upstream writes them, so `shadcn add --diff` shows only real
 * changes.
 */
const NCDAI_FILES = [
  "components/code-block-command.tsx",
  "components/copy-button.tsx",
  "components/icon-swap.tsx",
  "components/tabs.tsx",
  "hooks/use-copy-to-clipboard.ts",
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
      // `toaster` and `cn-toast` are hook classes from upstream, not Tailwind.
      files: ["components/ui/sonner.tsx"],
      rules: {
        "shadcn/no-unknown-classes": "off",
      },
    },
    {
      files: NCDAI_FILES,
      rules: {
        "eslint/func-style": "off",
        "eslint/no-use-before-define": "off",
        "eslint/sort-keys": "off",
        "react/function-component-definition": "off",
        "react/memo-dependencies": "off",
        "react/todo": "off",
        "shadcn/no-arbitrary-values": "off",
        "shadcn/no-raw-colors": "off",
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
        "components/home/**",
        "components/social/**",
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
                  "^(BarVisualizer|ElectricBarVisualizer|ElectricWaveform|SmoothWaveform|LiveWaveform|Spectrum|Waveform|DbReadout|DbScale|ClipIndicator|LevelMeter.*|VuMeter.*|Knob.*|Fader.*)$",
              },
              {
                allow: ["layout", "spacing", "shape", "color", "[--*"],
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
