import { defineConfig } from "react-doctor/api";

/**
 * React Doctor waivers, each checked against the code (plans/011). They stay
 * in this repo: registry files ship without them, so prefer fixing a finding
 * in source over adding it here.
 */
export default defineConfig({
  ignore: {
    overrides: [
      {
        // One file per component, exporting its cva variants and the helpers
        // the docs use (plan 002, rules 1 and 7). The detector also misreads
        // components that return useRender(...) as non-components.
        files: ["components/ui/**", "hooks/use-audio-context.tsx"],
        rules: ["react-doctor/only-export-components"],
      },
      {
        // Vendored from shadcn and chanhdai.com, and kept as upstream writes
        // them so `shadcn add --diff` shows only real changes.
        files: [
          "components/code-block-command.tsx",
          "components/copy-button.tsx",
          "components/icon-swap.tsx",
          "components/ui/field.tsx",
        ],
        rules: [
          "react-doctor/no-array-index-as-key",
          "react-doctor/only-export-components",
          "react-doctor/use-lazy-motion",
        ],
      },
      {
        // Each docs example stands alone, so its code tab copies whole.
        files: ["components/examples/**"],
        rules: ["react-doctor/duplicate-jsx-subtree"],
      },
      {
        // A fixed pool of three arcs, searched twice a frame.
        files: ["components/ui/electric-bar-visualizer.tsx"],
        rules: ["react-doctor/js-index-maps"],
      },
      {
        // A <meter> element can't hold the channel, bar and scale parts.
        files: ["components/ui/level-meter.tsx"],
        rules: ["react-doctor/prefer-tag-over-role"],
      },
      {
        // The hotkey handlers' deps are already the effect's, and they also
        // run from a JSX handler, which an effect event can't.
        files: ["components/ui/sound-pad.tsx"],
        rules: ["react-doctor/prefer-use-effect-event"],
      },
      {
        // Client-only decode into an AudioContext, behind a cancel flag and a
        // promise cache shared by every pad playing the same file.
        files: ["hooks/use-sound.ts"],
        rules: ["react-doctor/no-fetch-in-effect"],
      },
      {
        // The block owns the Web Audio graph, so the parent can't build the
        // output stream itself; onOutputChange hands it up.
        files: ["components/blocks/system-audio-mixer/system-audio-mixer.tsx"],
        rules: ["react-doctor/no-pass-data-to-parent"],
      },
      {
        // demo-audio caches a fixed set of track URLs for the page's life.
        // The soundboard revokes a URL once its removal can't be undone,
        // through state the detector can't follow.
        files: [
          "components/blocks/soundboard/soundboard.tsx",
          "lib/docs/demo-audio.ts",
        ],
        rules: ["react-doctor/no-create-object-url-without-revoke"],
      },
    ],
  },
});
