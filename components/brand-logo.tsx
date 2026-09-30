import Image from "next/image";

import logo from "@/public/brand/logo.png";

/** The wordmark supplies the accessible name; the dial is decorative. */
export const BrandLogo = () => (
  <Image
    alt=""
    aria-hidden
    className="size-8 shrink-0 dark:invert"
    data-slot="brand-logo"
    height={32}
    loading="eager"
    src={logo}
    width={32}
  />
);
