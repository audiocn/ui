"use client";

import { useTiks } from "@rexa-developer/tiks/react";
import { useCallback, useRef, useState } from "react";
import { useWebHaptics } from "web-haptics/react";

export type CopyState = "idle" | "done" | "error";

/** Text to copy, or a function returning it, which may resolve later. */
export type CopySource = string | (() => string | Promise<string>);

const TEXT_PLAIN = "text/plain";

const canWriteAsync = (): boolean =>
  typeof ClipboardItem !== "undefined" &&
  typeof navigator.clipboard?.write === "function";

/**
 * Safari only allows a clipboard write during the gesture that triggered it,
 * so a pending text is handed over as a promise inside a `ClipboardItem`
 * rather than awaited first.
 */
const writeText = async (pending: Promise<string>): Promise<string> => {
  if (!canWriteAsync()) {
    const text = await pending;
    await navigator.clipboard.writeText(text);
    return text;
  }

  const blob = (async () => new Blob([await pending], { type: TEXT_PLAIN }))();
  await navigator.clipboard.write([new ClipboardItem({ [TEXT_PLAIN]: blob })]);
  return await pending;
};

export interface UseCopyToClipboardOptions {
  onCopySuccess?: (text: string) => void;
  onCopyError?: (error: Error) => void;
  resetDelay?: number;
}

export function useCopyToClipboard({
  onCopySuccess,
  onCopyError,
  resetDelay = 1500,
}: UseCopyToClipboardOptions = {}) {
  const [state, setState] = useState<CopyState>("idle");
  const resetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { trigger: haptic } = useWebHaptics();
  const { success: tiksSuccess, error: tiksError } = useTiks();

  const copy = useCallback(
    async (text: CopySource) => {
      // Clear any pending reset
      if (resetTimeoutRef.current) {
        clearTimeout(resetTimeoutRef.current);
      }

      try {
        const value = typeof text === "function" ? text() : text;
        let finalText = value;

        if (typeof finalText === "string") {
          await navigator.clipboard.writeText(finalText);
        } else {
          finalText = await writeText(finalText);
        }

        setState("done");

        haptic("success");
        tiksSuccess();

        onCopySuccess?.(finalText);
      } catch (error) {
        setState("error");

        haptic("error");
        tiksError();

        onCopyError?.(
          error instanceof Error ? error : new Error("Copy failed")
        );
      } finally {
        // Schedule reset to idle
        resetTimeoutRef.current = setTimeout(() => {
          setState("idle");
        }, resetDelay);
      }
    },
    [onCopySuccess, onCopyError, haptic, tiksSuccess, tiksError, resetDelay]
  );

  return { copy, state } as const;
}
