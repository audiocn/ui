import Image from "next/image";

import { cn } from "@/lib/utils";
import logo from "@/public/brand/logo.png";

const DEFAULT_SIZE_PX = 32;

interface BrandLogoProps {
  className?: string;
  /** Rendered size in pixels, so larger marks stay sharp. Default 32. */
  size?: number;
}

/** The wordmark supplies the accessible name; the visualizer is decorative. */
export const BrandLogo = ({
  className,
  size = DEFAULT_SIZE_PX,
}: BrandLogoProps) => (
  <Image
    alt=""
    aria-hidden
    className={cn("size-8 shrink-0 dark:invert", className)}
    data-slot="brand-logo"
    height={size}
    loading="eager"
    src={logo}
    width={size}
  />
);
