"use client";

import { CheckIcon, CopyIcon } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";

const COPIED_MS = 2000;

/** A shell command with a button that copies it. */
export const CopyCommand = ({ command }: { command: string }) => {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) {
      return;
    }
    const timeout = setTimeout(() => setCopied(false), COPIED_MS);
    return () => {
      clearTimeout(timeout);
    };
  }, [copied]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
    } catch {
      // The clipboard is blocked; the command can still be selected by hand.
    }
  };

  return (
    <div className="bg-muted flex max-w-full items-center gap-1 rounded-lg py-1 pr-1 pl-3">
      <code className="min-w-0 overflow-x-auto font-mono text-xs whitespace-nowrap sm:text-sm">
        {command}
      </code>
      <Button
        aria-label="Copy install command"
        onClick={copy}
        size="icon-sm"
        variant="ghost"
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
      <span aria-live="polite" className="sr-only">
        {copied ? "Copied" : ""}
      </span>
    </div>
  );
};
