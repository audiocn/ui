"use client";

import { DesktopIcon, InfoIcon, WarningIcon } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

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
  const system = useSystemAudio();
  const { context } = useAudioContext();
  const [gainState, setGainState] = useState(DEFAULT_GAIN_DB);
  const gainDb = gainDbProp ?? gainState;
  const [gainNode, setGainNode] = useState<GainNode | null>(null);
  const analyser = useAudioAnalyser(gainNode, { channels: "stereo" });
  const active = system.status === "active";

  useEffect(() => {
    if (!(context && system.stream)) {
      setGainNode(null);
      onStreamChange?.(null);
      return;
    }
    const source = context.createMediaStreamSource(system.stream);
    const gain = context.createGain();
    const output = context.createMediaStreamDestination();
    source.connect(gain).connect(output);
    setGainNode(gain);
    onStreamChange?.(output.stream);
    return () => {
      source.disconnect();
      gain.disconnect();
    };
  }, [context, onStreamChange, system.stream]);

  useEffect(() => {
    if (gainNode && context) {
      gainNode.gain.setTargetAtTime(
        dbToGain(gainDb),
        context.currentTime,
        0.01
      );
    }
  }, [context, gainDb, gainNode]);

  const { start: startCapture, stop: stopCapture } = system;

  useEffect(() => {
    if (enabledProp === true) {
      startCapture();
    } else if (enabledProp === false) {
      stopCapture();
    }
  }, [enabledProp, startCapture, stopCapture]);

  const setEnabled = (next: boolean) => {
    onEnabledChange?.(next);
    if (enabledProp === undefined) {
      if (next) {
        system.start();
      } else {
        system.stop();
      }
    }
  };

  const setGain = (next: number) => {
    setGainState(next);
    onGainChange?.(next);
  };

  return (
    <Card className={className}>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <DesktopIcon />
          System audio
        </CardTitle>
        <CardDescription>
          Add the sound from your computer to your recording or stream.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <Field orientation="horizontal">
            <FieldContent>
              <FieldLabel htmlFor="system-audio-enabled">
                Capture system audio
              </FieldLabel>
              <FieldDescription>
                Your browser asks what to share. Choose a screen or tab and turn
                on its audio.
              </FieldDescription>
            </FieldContent>
            <Badge variant={active ? "default" : "secondary"}>
              {STATE_LABELS[system.status]}
            </Badge>
            <Switch
              checked={active || system.status === "prompting"}
              disabled={!system.isSupported}
              id="system-audio-enabled"
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
