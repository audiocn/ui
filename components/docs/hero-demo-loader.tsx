"use client";

import dynamic from "next/dynamic";

import { Skeleton } from "@/components/ui/skeleton";

const HeroDemo = dynamic(() => import("@/components/docs/hero-demo"), {
  loading: () => <Skeleton className="h-80 w-full" />,
  ssr: false,
});

export const HeroDemoLoader = () => <HeroDemo />;
