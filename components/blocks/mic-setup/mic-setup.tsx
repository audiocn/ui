"use client";

import {
  CheckCircleIcon,
  MicrophoneIcon,
  WarningCircleIcon,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AudioDeviceSelect,
  AudioDeviceSelectContent,
  AudioDeviceSelectPreview,
  AudioDeviceSelectTrigger,
  AudioDeviceSelectValue,
} from "@/components/ui/audio-device-select";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { DbReadout } from "@/components/ui/db-readout";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  LevelMeter,
  LevelMeterBar,
  LevelMeterChannel,
  LevelMeterChannels,
  LevelMeterClip,
  LevelMeterHold,
  LevelMeterScale,
  LevelMeterTrack,
} from "@/components/ui/level-meter";
import { LiveWaveform } from "@/components/ui/live-waveform";
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
import { useAudioDevices } from "@/hooks/use-audio-devices";
import { useMicrophone } from "@/hooks/use-microphone";
import { dbToGain } from "@/lib/audio/decibels";

const CHECK_DURATION_MS = 3000;

type CheckResult = "good" | "quiet" | "loud" | "silent";

const RESULTS: Record<
  CheckResult,
  { title: string; description: string; ok: boolean }
> = {
  good: {
    description: "Your level sits in the right range.",
    ok: true,
    title: "Sounds good",
  },
  loud: {
    description: "Your voice clipped. Lower the gain.",
    ok: false,
    title: "Too loud",
  },
  quiet: {
    description: "Raise the gain or move closer.",
    ok: false,
    title: "Too quiet",
  },
  silent: {
    description: "Check the device and that it is not muted.",
    ok: false,
    title: "No signal",
  },
};

const judge = (peakDb: number): CheckResult => {
  if (peakDb < -55) {
    return "silent";
  }
  if (peakDb >= -1) {
    return "loud";
  }
  if (peakDb < -30) {
    return "quiet";
  }
  return "good";
};

export interface MicSetupProps {
  deviceId?: string | null;
  onDeviceChange?: (deviceId: string | null) => void;
  gainDb?: number;
  onGainChange?: (gainDb: number) => void;
  muted?: boolean;
  onMutedChange?: (muted: boolean) => void;
  /** Open the microphone on mount instead of waiting for a click. Default false. */
  autoStart?: boolean;
  className?: string;
}

export const MicSetup = ({
  deviceId: deviceIdProp,
  onDeviceChange,
  gainDb: gainDbProp,
  onGainChange,
  muted: mutedProp,
  onMutedChange,
  autoStart = false,
  className,
}: MicSetupProps) => {
  const [started, setStarted] = useState(autoStart);
  const [deviceIdState, setDeviceIdState] = useState<string | null>(null);
  const [gainState, setGainState] = useState(0);
  const [mutedState, setMutedState] = useState(false);
  const deviceId = deviceIdProp === undefined ? deviceIdState : deviceIdProp;
  const gainDb = gainDbProp ?? gainState;
  const muted = mutedProp ?? mutedState;

  const devices = useAudioDevices();
  const microphone = useMicrophone({ deviceId, enabled: started });
  const { context } = useAudioContext();
  const gainNode = useMemo(() => context?.createGain() ?? null, [context]);
  const analyser = useAudioAnalyser(microphone.stream ? gainNode : null, {
    historySize: 120,
  });
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<CheckResult | null>(null);
  const peakRef = useRef(Number.NEGATIVE_INFINITY);

  useEffect(() => {
    if (!(context && gainNode && microphone.stream)) {
      return;
    }
    const source = context.createMediaStreamSource(microphone.stream);
    source.connect(gainNode);
    return () => source.disconnect();
  }, [context, gainNode, microphone.stream]);

  useEffect(() => {
    if (gainNode && context) {
      gainNode.gain.setTargetAtTime(
        muted ? 0 : dbToGain(gainDb),
        context.currentTime,
        0.01
      );
    }
  }, [context, gainDb, gainNode, muted]);

  useEffect(() => {
    if (!checking) {
      return;
    }
    peakRef.current = Number.NEGATIVE_INFINITY;
    const unsubscribe = analyser.meter.subscribe((frame) => {
      for (const level of frame.channels) {
        peakRef.current = Math.max(peakRef.current, level.peakDb);
      }
    });
    const timer = setTimeout(() => {
      setChecking(false);
      setResult(judge(peakRef.current));
    }, CHECK_DURATION_MS);
    return () => {
      unsubscribe();
      clearTimeout(timer);
    };
  }, [analyser.meter, checking]);

  const setDeviceId = (next: string | null) => {
    setDeviceIdState(next);
    onDeviceChange?.(next);
  };
  const setGain = (next: number) => {
    setGainState(next);
    onGainChange?.(next);
  };
  const setMuted = (next: boolean) => {
    setMutedState(next);
    onMutedChange?.(next);
  };

  const active = microphone.status === "active";
  const verdict = result ? RESULTS[result] : null;

  return (
    <Card className={className}>
      <CardHeader>
        <CardTitle>Microphone</CardTitle>
        <CardDescription>
          Pick a microphone and check your level.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup>
          <Field>
            <FieldLabel>Device</FieldLabel>
            <AudioDeviceSelect
              devices={devices.devices}
              loading={devices.isLoading}
              onRequestPermission={() => devices.requestPermission()}
              onValueChange={setDeviceId}
              permission={
                devices.permission === "unsupported"
                  ? "denied"
                  : devices.permission
              }
              value={deviceId}
            >
              <AudioDeviceSelectTrigger>
                <MicrophoneIcon className="text-muted-foreground" />
                <AudioDeviceSelectValue placeholder="Default microphone" />
              </AudioDeviceSelectTrigger>
              <AudioDeviceSelectContent />
            </AudioDeviceSelect>
            <AudioDeviceSelectPreview className="relative">
              {active ? null : (
                <Button
                  className="absolute inset-0 m-auto w-fit"
                  onClick={() => setStarted(true)}
                  size="xs"
                  variant="outline"
                >
                  Turn on microphone
                </Button>
              )}
              <LiveWaveform
                active={active && !muted}
                aria-label="Microphone preview"
                barWidth={2}
                className="h-10"
                mode="scrolling"
                source={analyser.visual}
              />
            </AudioDeviceSelectPreview>
          </Field>
          <Field>
            <div className="flex items-center justify-between">
              <FieldLabel>Level</FieldLabel>
              <DbReadout
                className="text-muted-foreground text-xs"
                holdMs={500}
                source={analyser.meter}
              />
            </div>
            <LevelMeter aria-label="Microphone level" source={analyser.meter}>
              <LevelMeterChannels>
                <LevelMeterChannel>
                  <LevelMeterTrack>
                    <LevelMeterBar />
                    <LevelMeterHold />
                  </LevelMeterTrack>
                </LevelMeterChannel>
                <LevelMeterScale />
              </LevelMeterChannels>
              <LevelMeterClip />
            </LevelMeter>
          </Field>
          <ParameterSlider
            max={24}
            min={-24}
            onValueChange={setGain}
            origin={0}
            resetValue={0}
            unit="dB"
            value={gainDb}
          >
            <ParameterSliderHeader>
              <ParameterSliderLabel>Gain</ParameterSliderLabel>
              <ParameterSliderReset />
              <ParameterSliderInput />
            </ParameterSliderHeader>
            <ParameterSliderControl />
          </ParameterSlider>
          <Field orientation="horizontal">
            <Switch
              checked={muted}
              id="mic-setup-mute"
              onCheckedChange={setMuted}
            />
            <FieldLabel htmlFor="mic-setup-mute">Mute microphone</FieldLabel>
          </Field>
          <div className="flex flex-col gap-3">
            <Button
              className="self-start"
              disabled={!active || checking}
              onClick={() => {
                setResult(null);
                setChecking(true);
              }}
              variant="outline"
            >
              {checking ? "Listening… say a few words" : "Check level"}
            </Button>
            {verdict ? (
              <Alert variant={verdict.ok ? "default" : "destructive"}>
                {verdict.ok ? <CheckCircleIcon /> : <WarningCircleIcon />}
                <AlertTitle>{verdict.title}</AlertTitle>
                <AlertDescription>{verdict.description}</AlertDescription>
              </Alert>
            ) : null}
            {microphone.status === "denied" ? (
              <Alert variant="destructive">
                <WarningCircleIcon />
                <AlertTitle>Microphone blocked</AlertTitle>
                <AlertDescription>
                  Allow microphone access in your browser&apos;s site settings.
                </AlertDescription>
              </Alert>
            ) : null}
          </div>
        </FieldGroup>
      </CardContent>
    </Card>
  );
};
