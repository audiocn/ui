import { ArrowUpRightIcon } from "@phosphor-icons/react/ssr";
import Link from "next/link";
import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

interface ShowcaseCardProps {
  /** The name on the card, like the tape label on a console channel. */
  label: string;
  /** The docs page the label links to. */
  href: string;
  className?: string;
  children: ReactNode;
}

/**
 * A showcase tile: a live component under a label that links to its docs.
 * Only the label is a link, so every control in the tile stays usable.
 */
export const ShowcaseCard = ({
  label,
  href,
  className,
  children,
}: ShowcaseCardProps) => (
  <article
    aria-label={label}
    className={cn(
      "bg-card text-card-foreground flex min-w-0 flex-col gap-4 rounded-xl border p-4",
      className
    )}
    data-slot="showcase-card"
  >
    <Link
      className="group/label text-muted-foreground hover:text-foreground focus-visible:ring-ring/50 -m-1 flex w-fit items-center gap-1 rounded-sm p-1 font-mono text-xs tracking-wider uppercase transition-colors outline-none focus-visible:ring-3"
      href={href}
    >
      {label}
      <span className="sr-only"> docs</span>
      <ArrowUpRightIcon
        aria-hidden
        className="size-3 transition-transform group-hover/label:translate-x-0.5 group-hover/label:-translate-y-0.5"
      />
    </Link>
    <div className="flex min-w-0 flex-1 flex-col items-center justify-center">
      {children}
    </div>
  </article>
);
