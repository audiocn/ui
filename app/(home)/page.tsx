import Link from "next/link";

import { HeroDemoLoader } from "@/components/docs/hero-demo-loader";
import { Button } from "@/components/ui/button";
import { siteConfig } from "@/lib/site";

const Page = () => (
  <main className="mx-auto flex w-full max-w-6xl flex-col px-4 py-16 sm:px-6 lg:py-24">
    <section className="grid grid-cols-[minmax(0,1fr)] items-center gap-12 lg:grid-cols-[minmax(0,1fr)_28rem]">
      <div className="flex min-w-0 flex-col items-start gap-6">
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
  </main>
);

export default Page;
