"use client";

import Link from "next/link";
import type { ComponentProps } from "react";

import { BrandAssetsMenu } from "@/components/brand-assets-menu";
import { BrandLogo } from "@/components/brand-logo";
import { siteConfig } from "@/lib/site";

const BrandNavTitle = ({
  size,
  href = "/",
  ...props
}: ComponentProps<"a"> & { size: 24 | 32 }) => (
  <BrandAssetsMenu>
    <Link {...props} href={href}>
      <span className="font-heading inline-flex items-center gap-1 font-semibold tracking-tight">
        <BrandLogo className={size === 24 ? "size-6" : "size-8"} size={size} />
        {siteConfig.name}
      </span>
    </Link>
  </BrandAssetsMenu>
);

export const DocsBrandNavTitle = (props: ComponentProps<"a">) => (
  <BrandNavTitle {...props} size={24} />
);

export const HomeBrandNavTitle = (props: ComponentProps<"a">) => (
  <BrandNavTitle {...props} size={32} />
);
