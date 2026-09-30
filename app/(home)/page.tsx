import { ArrowRightIcon } from "@phosphor-icons/react/ssr";
import Link from "next/link";

import { CopyCommand } from "@/components/home/copy-command";
import { HeroThreads } from "@/components/home/hero-threads";
import { HeroWaveform } from "@/components/home/hero-waveform";
import { ShowcaseGrid } from "@/components/home/showcase-grid";
import { ThemeSwatches } from "@/components/home/theme-swatches";
import { Button } from "@/components/ui/button";
import { siteConfig } from "@/lib/site";
import { source } from "@/lib/source";

const componentCount = source
  .getPages()
  .filter((page) => page.url.startsWith("/docs/components/")).length;

const Page = () => (
  <main className="relative isolate w-full">
    <div
      aria-hidden
      className="pointer-events-none absolute inset-x-0 top-0 -z-10 h-150 mask-b-from-60%"
    >
      <HeroThreads />
    </div>
    <div className="mx-auto flex w-full max-w-7xl flex-col px-4 pt-16 pb-24 sm:px-6 lg:pt-24">
      <section className="flex flex-col items-center gap-6 text-center">
        <Link
          className="text-muted-foreground hover:text-foreground hover:bg-muted/60 focus-visible:ring-ring/50 group/pill flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs transition-colors outline-none focus-visible:ring-3"
          href="/docs/components"
        >
          Audio components for shadcn/ui
          <ArrowRightIcon
            aria-hidden
            className="size-3 transition-transform group-hover/pill:translate-x-0.5"
          />
        </Link>
        <h1 className="font-heading max-w-4xl text-4xl font-semibold tracking-tight text-balance sm:text-6xl lg:text-7xl">
          Audio UI, mixed and mastered.
        </h1>
        <p className="text-muted-foreground max-w-2xl text-base text-balance sm:text-lg">
          Meters, faders, knobs, visualizers and a complete mixer for React.
          Built the shadcn way, so you own every line.
        </p>
        <div className="flex flex-wrap justify-center gap-3">
          <Button render={<Link href="/docs" />} nativeButton={false} size="lg">
            Get started
          </Button>
          <Button
            render={<Link href="/docs/components" />}
            nativeButton={false}
            size="lg"
            variant="outline"
          >
            Browse components
          </Button>
        </div>
        <CopyCommand
          command={`npx shadcn@latest add ${siteConfig.registryNamespace}/mixer`}
        />
      </section>
      <div className="my-12 lg:my-16">
        <HeroWaveform />
      </div>
      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-muted-foreground text-sm">
          Everything here is live. Drag a fader, turn a knob, hit a pad.
        </p>
        <ThemeSwatches />
      </div>
      <ShowcaseGrid />
      <div className="mt-12 flex justify-center">
        <Button
          render={<Link href="/docs/components" />}
          nativeButton={false}
          size="lg"
          variant="outline"
        >
          Browse all {componentCount} components
          <ArrowRightIcon data-icon="inline-end" />
        </Button>
      </div>
    </div>
  </main>
);

export default Page;
