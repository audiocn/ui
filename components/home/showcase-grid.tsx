import { ShowcaseCard } from "@/components/home/showcase-card";
import { ShowcaseTile } from "@/components/home/showcase-tile";
import type { ShowcaseTileName } from "@/components/home/showcase-tile";
import { cn } from "@/lib/utils";

interface ShowcaseItem {
  tile: ShowcaseTileName;
  label: string;
  href: string;
}

const WIDE: ShowcaseItem[] = [
  {
    href: "/docs/components/mixer",
    label: "Mixer",
    tile: "mixer",
  },
  {
    href: "/docs/components/waveform",
    label: "Waveform",
    tile: "waveform",
  },
  {
    href: "/docs/blocks/music-player",
    label: "Music player",
    tile: "music",
  },
  {
    href: "/docs/components/sound-pad",
    label: "Sound pads",
    tile: "sound-pads",
  },
];

const LEFT: ShowcaseItem[] = [
  {
    href: "/docs/components/bar-visualizer",
    label: "Bar visualizer",
    tile: "voice",
  },
  { href: "/docs/components/knob", label: "Knobs", tile: "knobs" },
  { href: "/docs/components/spectrum", label: "Spectrum", tile: "spectrum" },
  {
    href: "/docs/components/parameter-slider",
    label: "Parameter sliders",
    tile: "eq",
  },
  {
    href: "/docs/components/live-waveform",
    label: "Live waveform",
    tile: "live-waveform",
  },
];

const RIGHT: ShowcaseItem[] = [
  {
    href: "/docs/components/level-meter",
    label: "Level meters",
    tile: "meters",
  },
  {
    href: "/docs/components/channel-toggle",
    label: "Channel controls",
    tile: "channel",
  },
  { href: "/docs/components/fader", label: "Faders", tile: "faders" },
  {
    href: "/docs/components/audio-device-select",
    label: "Output",
    tile: "output",
  },
  {
    href: "/docs/components/audio-player",
    label: "Audio player",
    tile: "compact-player",
  },
];

const Column = ({
  items,
  className,
}: {
  items: ShowcaseItem[];
  className?: string;
}) => (
  <div className={cn("flex min-w-0 flex-col gap-4", className)}>
    {items.map((item) => (
      <ShowcaseCard href={item.href} key={item.tile} label={item.label}>
        <ShowcaseTile name={item.tile} />
      </ShowcaseCard>
    ))}
  </div>
);

/**
 * Three stacks of live tiles: narrow, wide, narrow on desktop. The wide stack
 * comes first in the markup, so phones and tablets show the mixer first.
 */
export const ShowcaseGrid = () => (
  <section
    aria-label="Live components"
    className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4"
  >
    <Column className="md:col-span-2 lg:order-2" items={WIDE} />
    <Column className="lg:order-1" items={LEFT} />
    <Column className="lg:order-3" items={RIGHT} />
  </section>
);
