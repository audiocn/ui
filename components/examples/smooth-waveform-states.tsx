import { SmoothWaveform } from "@/components/ui/smooth-waveform";

const SmoothWaveformStates = () => (
  <div className="grid w-full max-w-lg gap-6 sm:grid-cols-2">
    <div className="grid gap-2">
      <SmoothWaveform aria-label="Idle" className="text-primary h-20" />
      <span className="text-muted-foreground text-center text-xs">idle</span>
    </div>
    <div className="grid gap-2">
      <SmoothWaveform
        aria-label="Connecting"
        className="text-primary h-20"
        loading
      />
      <span className="text-muted-foreground text-center text-xs">loading</span>
    </div>
  </div>
);

export default SmoothWaveformStates;
