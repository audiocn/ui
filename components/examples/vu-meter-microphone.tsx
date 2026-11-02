"use client";

import { MicrophoneIcon, MicrophoneSlashIcon } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { VuMeter } from "@/components/ui/vu-meter";
import { useMicrophone } from "@/hooks/use-microphone";

const VuMeterMicrophone = () => {
  const microphone = useMicrophone();
  const listening = microphone.status === "active";

  return (
    <div className="flex w-full max-w-md flex-col gap-4">
      <VuMeter aria-label="Microphone level" input={microphone.stream} />
      <Button
        className="self-start"
        onClick={listening ? microphone.stop : microphone.start}
        size="sm"
        variant="outline"
      >
        {listening ? (
          <MicrophoneSlashIcon data-icon="inline-start" />
        ) : (
          <MicrophoneIcon data-icon="inline-start" />
        )}
        {listening ? "Stop microphone" : "Use my microphone"}
      </Button>
    </div>
  );
};

export default VuMeterMicrophone;
