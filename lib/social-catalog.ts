export type SocialPreviewName =
  | "home"
  | "mixer"
  | "meters"
  | "knobs"
  | "waveform"
  | "electric-waveform"
  | "bars"
  | "electric-bars"
  | "live-waveform"
  | "smooth-waveform"
  | "spectrum"
  | "faders"
  | "parameters"
  | "tempo"
  | "pan"
  | "volume"
  | "channel"
  | "toggles"
  | "readout"
  | "scale"
  | "clip"
  | "pads"
  | "devices"
  | "player"
  | "tracks"
  | "system-mixer"
  | "mic-setup"
  | "system-settings"
  | "quick-popover"
  | "soundboard"
  | "music-player"
  | "collection"
  | "blocks"
  | "theming";

export interface SocialCardDefinition {
  id: string;
  pathname: string;
  title: string;
  category: string;
  caption: string;
  alt: string;
  preview: SocialPreviewName;
}

// The development capture endpoint derives the catalog from Fumadocs pages.
const featuredCards: SocialCardDefinition[] = [
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

export const componentPreviews: Record<string, SocialPreviewName | undefined> =
  {
    "audio-device-select": "devices",
    "audio-player": "player",
    "bar-visualizer": "bars",
    "channel-strip": "channel",
    "channel-toggle": "toggles",
    "clip-indicator": "clip",
    "db-readout": "readout",
    "db-scale": "scale",
    "electric-bar-visualizer": "electric-bars",
    "electric-waveform": "electric-waveform",
    fader: "faders",
    knob: "knobs",
    "level-meter": "meters",
    "live-waveform": "live-waveform",
    mixer: "mixer",
    "pan-control": "pan",
    "parameter-slider": "parameters",
    "smooth-waveform": "smooth-waveform",
    "sound-pad": "pads",
    spectrum: "spectrum",
    "tempo-control": "tempo",
    "track-list": "tracks",
    "volume-control": "volume",
    waveform: "waveform",
  };

const blockPreviews: Record<string, SocialPreviewName | undefined> = {
  "mic-setup": "mic-setup",
  "music-player": "music-player",
  "quick-audio-popover": "quick-popover",
  soundboard: "soundboard",
  "system-audio-mixer": "system-mixer",
  "system-audio-settings": "system-settings",
};

const contextualPreviews: Record<string, SocialPreviewName | undefined> = {
  "/docs": "collection",
  "/docs/blocks": "blocks",
  "/docs/components": "collection",
  "/docs/concepts/accessibility": "knobs",
  "/docs/concepts/custom-engine": "mixer",
  "/docs/concepts/decibels": "meters",
  "/docs/concepts/feeding-data": "spectrum",
  "/docs/concepts/theming": "theming",
  "/docs/hooks/use-audio-analyser": "spectrum",
  "/docs/hooks/use-audio-context": "mixer",
  "/docs/hooks/use-audio-devices": "devices",
  "/docs/hooks/use-audio-player": "player",
  "/docs/hooks/use-clip-hold": "clip",
  "/docs/hooks/use-demo-signal": "bars",
  "/docs/hooks/use-frame-source": "live-waveform",
  "/docs/hooks/use-gain-node": "channel",
  "/docs/hooks/use-level": "meters",
  "/docs/hooks/use-microphone": "mic-setup",
  "/docs/hooks/use-mixer": "mixer",
  "/docs/hooks/use-reduced-motion": "smooth-waveform",
  "/docs/hooks/use-sound": "pads",
  "/docs/hooks/use-system-audio": "system-settings",
  "/docs/hooks/use-tap-tempo": "tempo",
  "/docs/hooks/use-visibility": "bars",
  "/docs/hooks/use-waveform-data": "waveform",
  "/docs/hooks/use-web-audio-mixer": "system-mixer",
  "/docs/installation": "collection",
};

const previewDescriptions: Record<SocialPreviewName, string> = {
  bars: "a frequency-band bar visualizer with an active signal",
  blocks: "a mixer and sound pads composed into an audio console",
  channel: "a channel strip with a meter, fader and channel controls",
  clip: "a latched clip light beside a level meter",
  collection: "level meters, rotary knobs, faders and a waveform",
  devices: "microphone selectors with named USB and default devices",
  "electric-bars": "glowing frequency bars drawn as electric filaments",
  "electric-waveform":
    "an electric waveform with a bright core and branching arcs",
  faders: "three console faders at different gain settings",
  home: "a mixer, rotary knobs and a waveform",
  knobs: "gain, pan and frequency knobs with labelled values",
  "live-waveform": "a live oscilloscope trace and scrolling signal history",
  meters: "solid and segmented stereo level meters with scales and peak hold",
  "mic-setup":
    "microphone setup with a device selector, preview, gain and level check",
  mixer: "a five-channel mixer with meters, faders and mute and solo controls",
  "music-player":
    "a complete playlist player with waveform seeking and transport controls",
  pads: "sound pads with hotkeys and playback progress",
  pan: "a pan slider with a centre detent and a left balance value",
  parameters: "frequency, Q and gain parameter sliders",
  player: "an audio player with transport, seeking, time and volume controls",
  "quick-popover":
    "an open audio popover with microphone and system audio settings",
  readout: "decibel readouts for quiet, normal and hot signal levels",
  scale: "a decibel scale alongside a console fader",
  "smooth-waveform": "a flowing audio wave and an oscilloscope line",
  soundboard:
    "a complete soundboard with hotkeys, pads, volume and stop controls",
  spectrum: "a frequency spectrum with axes, grid and peak hold",
  "system-mixer":
    "a complete system audio mixer with microphone, music, sound pads and master controls",
  "system-settings": "system audio settings with capture, volume and a meter",
  tempo: "an editable BPM input and a tap tempo button",
  theming: "the same audio controls in light and dark themes",
  toggles: "mute, solo and monitor controls with their pressed states",
  tracks: "a track list with active and playing states",
  volume: "a volume slider with a mute control and numeric value",
  waveform: "a mirrored waveform with a playhead, selected region and marker",
};

interface SocialPage {
  url: string;
  data: { title: string; description?: string };
}

const previewForPage = (pathname: string): SocialPreviewName => {
  const [section, slug] = pathname.split("/").slice(2);
  if (section === "components" && slug) {
    return componentPreviews[slug] ?? "collection";
  }
  if (section === "blocks" && slug) {
    return blockPreviews[slug] ?? "blocks";
  }
  return contextualPreviews[pathname] ?? "collection";
};

const categoryForPage = (pathname: string) => {
  if (pathname.startsWith("/docs/components/")) {
    return "Component / React";
  }
  if (pathname.startsWith("/docs/blocks/")) {
    return "Block / React";
  }
  if (pathname.startsWith("/docs/hooks/")) {
    return "Hook / React";
  }
  return "Documentation / React";
};

export const getSocialCards = (pages: SocialPage[]): SocialCardDefinition[] => {
  const home = featuredCards.find((card) => card.pathname === "/");
  if (!home) {
    throw new Error("The social catalog must define a homepage card.");
  }
  return [
    home,
    {
      alt: "audiocn contributors: a mixer and its controls, built together in the open.",
      caption: "Open source audio UI. Built together, one component at a time.",
      category: "Community / Open source",
      id: "contributors",
      pathname: "/contributors",
      preview: "home",
      title: "Built together.",
    },
    ...pages.map((page) => {
      const featured = featuredCards.find((card) => card.pathname === page.url);
      if (featured) {
        return featured;
      }
      const preview = previewForPage(page.url);
      return {
        alt: `${page.data.title} — audiocn: ${previewDescriptions[preview]}.`,
        caption:
          page.data.description ??
          "Composable audio components for React and shadcn/ui.",
        category: categoryForPage(page.url),
        id:
          page.url === "/docs"
            ? "introduction"
            : page.url.slice(6).replaceAll("/", "-"),
        pathname: page.url,
        preview,
        title: page.data.title,
      };
    }),
  ];
};
