"use client";

import { DesktopIcon, InfoIcon, WarningIcon } from "@phosphor-icons/react";
import { useEffect, useEffectEvent, useId, useMemo, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { LevelMeter } from "@/components/ui/level-meter";
import {
  ParameterSlider,
  ParameterSliderControl,
  ParameterSliderHeader,
  ParameterSliderInput,
  ParameterSliderLabel,
  ParameterSliderReset,
} from "@/components/ui/parameter-slider";
import { Switch } from "@/components/ui/switch";
import { useAudioAnalyser } from "@/hooks/use-audio-analyser";
import { useAudioContext } from "@/hooks/use-audio-context";
import { useGainNode } from "@/hooks/use-gain-node";
import { useSystemAudio } from "@/hooks/use-system-audio";
import type { SystemAudioStatus } from "@/hooks/use-system-audio";
import { dbToGain } from "@/lib/audio/decibels";

const DEFAULT_GAIN_DB = -6;

const STATE_LABELS: Record<SystemAudioStatus, string> = {
  active: "On",
  denied: "Needs permission",
  ended: "Ended",
  idle: "Off",
  "no-audio": "No audio shared",
  prompting: "Choose what to share…",
  unsupported: "Unsupported",
};

export interface SystemAudioSettingsProps {
  enabled?: boolean;
  onEnabledChange?: (enabled: boolean) => void;
  gainDb?: number;
  onGainChange?: (gainDb: number) => void;
  /** The captured stream after the level control, for your own routing. */
  onStreamChange?: (stream: MediaStream | null) => void;
  className?: string;
}

export const SystemAudioSettings = ({
  enabled: enabledProp,
  onEnabledChange,
  gainDb: gainDbProp,
  onGainChange,
  onStreamChange,
  className,
}: SystemAudioSettingsProps) => {
  const enabledId = useId();
  const system = useSystemAudio();
  const { context } = useAudioContext();
  const [gainState, setGainState] = useState(DEFAULT_GAIN_DB);
  const gainDb = gainDbProp ?? gainState;
  const output = useMemo(
    () => context?.createMediaStreamDestination() ?? null,
    [context]
  );
  // The captured stream, through the level control, into a stream for the
  // parent. Not routed to the speakers.
  const gainNode = useGainNode({
    destination: output,
    gain: dbToGain(gainDb),
    input: system.stream,
  });
  const analyser = useAudioAnalyser(system.stream ? gainNode : null, {
    channels: "stereo",
  });
  const active = system.status === "active";

  // A new callback each parent render must not re-emit the same stream.
  const emitStream = useEffectEvent((stream: MediaStream | null) => {
    onStreamChange?.(stream);
  });
  const processed =
    context && gainNode && output && system.stream ? output.stream : null;

  useEffect(() => {
    emitStream(processed);
  }, [processed]);

  const { start: startCapture, stop: stopCapture } = system;
  const isCapturing = useEffectEvent(
    () => system.status === "active" || system.status === "prompting"
  );

  useEffect(() => {
    if (enabledProp === true) {
      if (!isCapturing()) {
        startCapture();
      }
    } else if (enabledProp === false) {
      stopCapture();
    }
  }, [enabledProp, startCapture, stopCapture]);

  const setEnabled = (next: boolean) => {
    onEnabledChange?.(next);
    // Controlled, the effect above acts once the parent passes the new value.
    // When the prop already says so (capture ended while `enabled` stayed
    // true), nothing will change, so act here.
    const propWillChange = enabledProp !== undefined && enabledProp !== next;
    if (propWillChange) {
      return;
    }
    if (next) {
      system.start();
    } else {
      system.stop();
    }
  };

  const setGain = (next: number) => {
    setGainState(next);
    onGainChange?.(next);
  };

  return (
    <Card className={className}>
      <CardHeader>
        <CardTitle>
          <span className="flex items-center gap-2">
            <DesktopIcon />
            System audio
          </span>
        </CardTitle>
        <CardDescription>
          Add the sound from your computer to your recording or stream.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <Field orientation="horizontal">
            <FieldContent>
              <div className="flex flex-wrap items-center gap-2">
                <FieldLabel htmlFor={enabledId}>
                  Capture system audio
                </FieldLabel>
                <Badge variant={active ? "default" : "secondary"}>
                  {STATE_LABELS[system.status]}
                </Badge>
              </div>
              <FieldDescription>
                Your browser asks what to share. Choose a screen or tab and turn
                on its audio.
              </FieldDescription>
            </FieldContent>
            <Switch
              checked={active || system.status === "prompting"}
              disabled={!system.isSupported}
              id={enabledId}
              onCheckedChange={setEnabled}
            />
          </Field>
          <ParameterSlider
            disabled={!active}
            max={12}
            min={-24}
            onValueChange={setGain}
            origin={0}
            resetValue={DEFAULT_GAIN_DB}
            unit="dB"
            value={gainDb}
          >
            <ParameterSliderHeader>
              <ParameterSliderLabel>Level</ParameterSliderLabel>
              <ParameterSliderReset />
              <ParameterSliderInput />
            </ParameterSliderHeader>
            <ParameterSliderControl />
          </ParameterSlider>
          {active ? (
            <LevelMeter
              aria-label="System audio level"
              channelCount={2}
              source={analyser.meter}
            />
          ) : null}
          {system.isSupported ? (
            <Alert>
              <InfoIcon />
              <AlertTitle>What gets captured</AlertTitle>
              <AlertDescription>
                Everything the shared screen, window or tab plays. Some browsers
                only offer tab audio.
              </AlertDescription>
            </Alert>
          ) : (
            <Alert variant="destructive">
              <WarningIcon />
              <AlertTitle>Not available in this browser</AlertTitle>
              <AlertDescription>
                This browser cannot capture system audio. Try a Chromium-based
                browser.
              </AlertDescription>
            </Alert>
          )}
          {system.status === "denied" ? (
            <Alert variant="destructive">
              <WarningIcon />
              <AlertTitle>Capture was not started</AlertTitle>
              <AlertDescription>
                No screen or tab was shared. Turn capture on again and approve
                the browser&apos;s sharing request. If access is blocked, allow
                screen sharing in your browser or system settings.
              </AlertDescription>
            </Alert>
          ) : null}
          {system.status === "no-audio" ? (
            <Alert variant="destructive">
              <WarningIcon />
              <AlertTitle>No audio was shared</AlertTitle>
              <AlertDescription>
                Turn capture on again and tick the option to share audio.
              </AlertDescription>
            </Alert>
          ) : null}
        </FieldGroup>
      </CardContent>
    </Card>
  );
};
