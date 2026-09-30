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
];

const RIGHT: ShowcaseItem[] = [];

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
