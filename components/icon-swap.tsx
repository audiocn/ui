"use client";

import type { AnimatePresenceProps, HTMLMotionProps } from "motion/react";
import { AnimatePresence, motion } from "motion/react";

export function IconSwap(props: React.PropsWithChildren<AnimatePresenceProps>) {
  return <AnimatePresence mode="popLayout" initial={false} {...props} />;
}

type MotionElement = typeof motion.div | typeof motion.span;

export function IconSwapItem({
  as: Component = motion.div,
  ...props
}: HTMLMotionProps<"div"> & {
  as?: MotionElement;
}) {
  return (
    <Component
      initial={{ filter: "blur(4px)", opacity: 0, scale: 0.25 }}
      animate={{ filter: "blur(0px)", opacity: 1, scale: 1 }}
      exit={{ filter: "blur(4px)", opacity: 0, scale: 0.25 }}
      transition={{
        bounce: 0,
        duration: 0.3,
        type: "spring",
      }}
      {...props}
    />
  );
}
