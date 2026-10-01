"use client";

import {
  CodeBlockCommand,
  convertNpmCommand,
} from "@/components/code-block-command";
import { cn } from "@/lib/utils";

interface InstallCommandProps {
  className?: string;
  /** An npm or npx command, shown translated for each package manager. */
  command: string;
}

/** An install command with a package manager switcher and a copy button. */
export const InstallCommand = ({ className, command }: InstallCommandProps) => (
  // Prose styles would box the command's <code> like inline code.
  <div className={cn("not-prose", className)}>
    <CodeBlockCommand {...convertNpmCommand(command)} />
  </div>
);
