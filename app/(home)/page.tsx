import Link from "next/link";

import { HeroDemoLoader } from "@/components/docs/hero-demo-loader";
import { Button } from "@/components/ui/button";
import { siteConfig } from "@/lib/site";

const features = [
  {
    description:
      "Level meters with peak hold and zones, bar visualizers, live and static waveforms, and a spectrum analyser.",
    title: "Meters and visualizers",
  },
  {
    description:
      "Faders in decibels with console tapers, parameter sliders, knobs, pan, mute, solo and monitor toggles.",
    title: "Controls",
  },
  {
    description:
      "Channel strips and a mixer, with a Web Audio engine for gain, ducking, monitoring and a limited master.",
    title: "A complete mixer",
  },
  {
    description:
      "A composable audio player, track lists and sound pads with hotkeys, for music and sound effects.",
    title: "Sounds and music",
  },
  {
    description:
      "Built on Base UI, styled with Tailwind, and themed with your shadcn tokens. You own every line.",
    title: "The shadcn way",
  },
  {
    description:
      "Meters paint on the animation frame without re-rendering React, so a mixer with many channels stays smooth.",
    title: "Fast by default",
  },
];

const Page = () => (
  <main className="mx-auto flex w-full max-w-6xl flex-col gap-20 px-4 py-16 sm:px-6 lg:py-24">
    <section className="grid items-center gap-12 lg:grid-cols-[1fr_28rem]">
      <div className="flex flex-col items-start gap-6">
        <span className="text-muted-foreground rounded-full border px-3 py-1 text-xs">
          Audio components for shadcn/ui
        </span>
        <h1 className="font-heading text-4xl font-semibold tracking-tight sm:text-5xl">
          Meters, faders and a complete mixer, built the shadcn way.
        </h1>
        <p className="text-muted-foreground max-w-xl text-lg">
          {siteConfig.description}
        </p>
        <div className="flex flex-wrap gap-3">
          <Button render={<Link href="/docs" />} nativeButton={false} size="lg">
            Get started
          </Button>
          <Button
            render={<Link href="/docs/blocks/system-audio-mixer" />}
            nativeButton={false}
            size="lg"
            variant="outline"
          >
            See the mixer
          </Button>
        </div>
        <code className="bg-muted max-w-full overflow-x-auto rounded-lg px-3 py-2 font-mono text-xs whitespace-nowrap sm:text-sm">
          npx shadcn@latest add @audiocn/level-meter
        </code>
      </div>
      <HeroDemoLoader />
    </section>
    <section className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
      {features.map((feature) => (
        <div
          className="flex flex-col gap-2 rounded-xl border p-5"
          key={feature.title}
        >
          <h2 className="font-heading font-medium">{feature.title}</h2>
          <p className="text-muted-foreground text-sm">{feature.description}</p>
        </div>
      ))}
    </section>
  </main>
);

export default Page;
