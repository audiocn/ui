"use client";

import { CheckIcon, CopyIcon, XCircleIcon } from "@phosphor-icons/react";
import { motion } from "motion/react";
import type { ComponentProps } from "react";

import { IconSwap, IconSwapItem } from "@/components/icon-swap";
import { Button } from "@/components/ui/button";
import type { CopySource, CopyState } from "@/hooks/use-copy-to-clipboard";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import { cn } from "@/lib/utils";

export interface CopyStateIconProps {
  state: CopyState;
  /** Custom icon for idle state. */
  idleIcon?: React.ReactNode;
  /** Custom icon for done state. */
  doneIcon?: React.ReactNode;
  /** Custom icon for error state. */
  errorIcon?: React.ReactNode;
}

export function CopyStateIcon({
  state,
  idleIcon,
  doneIcon,
  errorIcon,
}: CopyStateIconProps) {
  return (
    <IconSwap>
      <IconSwapItem key={state} as={motion.span}>
        {state === "idle" && (idleIcon ?? <CopyIcon data-slot="idle-icon" />)}

        {state === "done" && (doneIcon ?? <CheckIcon data-slot="done-icon" />)}

        {state === "error" &&
          (errorIcon ?? <XCircleIcon data-slot="error-icon" />)}
      </IconSwapItem>
    </IconSwap>
  );
}

export type CopyButtonProps = ComponentProps<typeof Button> & {
  /** The text to copy, or a function that returns it, possibly later. */
  text: CopySource;
  /** Called with the copied text on successful copy. */
  onCopySuccess?: (text: string) => void;
  /** Called with the error if the copy operation fails. */
  onCopyError?: (error: Error) => void;
} & Omit<CopyStateIconProps, "state">;

export function CopyButton({
  className,
  size = "icon",
  children,
  text,
  idleIcon,
  doneIcon,
  errorIcon,
  onClick,
  onCopySuccess,
  onCopyError,
  ...props
}: CopyButtonProps) {
  const { state, copy } = useCopyToClipboard({
    onCopyError,
    onCopySuccess,
  });

  return (
    <Button
      className={cn("will-change-transform", className)}
      size={size}
      onClick={(e) => {
        copy(text);
        onClick?.(e);
      }}
      aria-label="Copy"
      {...props}
    >
      <CopyStateIcon
        state={state}
        idleIcon={idleIcon}
        doneIcon={doneIcon}
        errorIcon={errorIcon}
      />
      {children}
    </Button>
  );
}
