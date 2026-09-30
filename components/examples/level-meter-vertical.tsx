"use client";

import {
  LevelMeter,
  LevelMeterBar,
  LevelMeterChannel,
  LevelMeterChannels,
  LevelMeterHold,
  LevelMeterScale,
  LevelMeterTrack,
  LevelMeterValue,
} from "@/components/ui/level-meter";
import { useDemoSignal } from "@/hooks/use-demo-signal";

const channels = [0, 1];

const LevelMeterVertical = () => {
  const signal = useDemoSignal({ channels: 2, kind: "music", seed: 4 });

  return (
    <LevelMeter
      aria-label="Program level"
      className="h-56"
      orientation="vertical"
      size="lg"
      source={signal.meter}
    >
      <LevelMeterChannels>
        {channels.map((index) => (
          <LevelMeterChannel index={index} key={index}>
            <LevelMeterTrack>
              <LevelMeterBar />
              <LevelMeterHold />
            </LevelMeterTrack>
          </LevelMeterChannel>
        ))}
        <LevelMeterScale />
      </LevelMeterChannels>
      <LevelMeterValue />
    </LevelMeter>
  );
};

export default LevelMeterVertical;
