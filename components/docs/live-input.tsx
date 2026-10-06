"use client";

import { MicrophoneIcon } from "@phosphor-icons/react";
import {
  createContext,
  useContext,
  useEffect,
  useId,
  useMemo,
  useState,
} from "react";
import type { ReactNode } from "react";
import { toast } from "sonner";

import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { getSharedAudioContext } from "@/hooks/use-audio-context";
import { DemoSignalProvider } from "@/hooks/use-demo-signal";
import { useMicrophone } from "@/hooks/use-microphone";
import type { MicrophoneStatus } from "@/hooks/use-microphone";
import { cn } from "@/lib/utils";

interface LiveInputState {
  enabled: boolean;
  setEnabled: (enabled: boolean) => void;
}

const LiveInputContext = createContext<LiveInputState | null>(null);

const FAILURES: Partial<Record<MicrophoneStatus, string>> = {
  denied:
    "Microphone access is blocked. Allow it in your browser's site settings to try again.",
  error: "The microphone could not be opened.",
  unavailable: "No microphone is available.",
};

/** Browsers start audio only from a gesture, so resume while it is handled. */
const resumeAudio = async () => {
  try {
    await getSharedAudioContext()?.resume();
  } catch {
    // The analysers still connect; the next gesture resumes the context.
  }
};

/**
 * Site-wide: when switched on, every preview built on a demo signal meters the
 * visitor's microphone instead.
 */
export const LiveInputProvider = ({ children }: { children: ReactNode }) => {
  const [enabled, setEnabled] = useState(false);
  const [failure, setFailure] = useState<{ message: string } | null>(null);
  const { status, stream } = useMicrophone({ enabled });

  // A refused or lost microphone turns the switch back off.
  const message = enabled ? FAILURES[status] : undefined;
  if (message) {
    setEnabled(false);
    setFailure({ message });
  }

  useEffect(() => {
    if (failure) {
      toast.error(failure.message);
    }
  }, [failure]);

  // The shared context only exists once the switch has been turned on.
  const input = useMemo(() => {
    const context = stream ? getSharedAudioContext() : null;
    return stream && context ? context.createMediaStreamSource(stream) : null;
  }, [stream]);

  const state = useMemo<LiveInputState>(
    () => ({
      enabled,
      setEnabled: (next) => {
        if (next) {
          resumeAudio();
        }
        setEnabled(next);
      },
    }),
    [enabled]
  );

  return (
    <LiveInputContext.Provider value={state}>
      <DemoSignalProvider input={input}>{children}</DemoSignalProvider>
    </LiveInputContext.Provider>
  );
};

/** Navbar switch for the site-wide live input. */
export const LiveInputSwitch = ({ className }: { className?: string }) => {
  const id = useId();
  const live = useContext(LiveInputContext);

  if (!live) {
    return null;
  }
  const { enabled, setEnabled } = live;

  return (
    <div className={cn("flex items-center gap-2 px-2", className)}>
      <Label htmlFor={id}>
        <MicrophoneIcon aria-hidden className="size-4" />
        Mic
      </Label>
      <Tooltip>
        <TooltipTrigger
          render={
            <Switch
              checked={enabled}
              id={id}
              onCheckedChange={setEnabled}
              size="sm"
            />
          }
        />
        <TooltipContent>Drive the previews with your microphone</TooltipContent>
      </Tooltip>
    </div>
  );
};
