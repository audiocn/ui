export type SocialPreviewName =
  | "home"
  | "mixer"
  | "meters"
  | "knobs"
  | "waveform"
  | "electric-waveform";

export interface SocialCardDefinition {
  id: string;
  pathname: string;
  title: string;
  category: string;
  caption: string;
  alt: string;
  preview: SocialPreviewName;
}

// This catalog also runs in the Node capture script; keep it free of app imports.
export const socialCards: SocialCardDefinition[] = [
  {
    alt: "audiocn: a five-channel mixer, rotary knobs and a waveform in the dark Stone theme.",
    caption:
      "Audio components for React and shadcn/ui. Copy, paste, make them yours.",
    category: "Audio components for React",
    id: "home",
    pathname: "/",
    preview: "home",
    title: "Audio UI, mixed and mastered.",
  },
  {
    alt: "audiocn Mixer: microphone, system audio, music, sounds and master strips with faders and active meters.",
    caption:
      "Channel strips, shared metering and keyboard navigation. Your console, your code.",
    category: "Component / React",
    id: "mixer",
    pathname: "/docs/components/mixer",
    preview: "mixer",
    title: "Mixer",
  },
  {
    alt: "audiocn Level Meter: solid and segmented stereo meters with green and yellow zones, scales and peak hold.",
    caption: "Peak and RMS. Stereo channels. Peak hold and clip detection.",
    category: "Component / React",
    id: "level-meter",
    pathname: "/docs/components/level-meter",
    preview: "meters",
    title: "Level Meter",
  },
  {
    alt: "audiocn Knob: gain, pan and frequency rotary controls with value labels and range arcs.",
    caption:
      "A rotary control with drag, keyboard input and an editable value.",
    category: "Component / React",
    id: "knob",
    pathname: "/docs/components/knob",
    preview: "knobs",
    title: "Knob",
  },
  {
    alt: "audiocn Waveform: an audio clip with a mirrored waveform, a playhead, a selected region and a marker.",
    caption:
      "Every peak in view. Seek, select regions and mark the moments that matter.",
    category: "Component / React",
    id: "waveform",
    pathname: "/docs/components/waveform",
    preview: "waveform",
    title: "Waveform",
  },
  {
    alt: "audiocn Electric Waveform: a glowing audio trace with a bright core and branching arcs on a dark surface.",
    caption: "A white-hot core. Branching arcs. A signal you can feel.",
    category: "Component / React",
    id: "electric-waveform",
    pathname: "/docs/components/electric-waveform",
    preview: "electric-waveform",
    title: "Electric Waveform",
  },
];
