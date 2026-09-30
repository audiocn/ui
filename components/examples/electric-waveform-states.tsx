import { ElectricWaveform } from "@/components/ui/electric-waveform";

const ElectricWaveformStates = () => (
  <div className="grid w-full max-w-lg gap-6 sm:grid-cols-2">
    <div className="grid gap-2">
      <ElectricWaveform aria-label="Idle" className="text-primary h-20" />
      <span className="text-muted-foreground text-center text-xs">idle</span>
    </div>
    <div className="grid gap-2">
      <ElectricWaveform
        aria-label="Connecting"
        className="text-primary h-20"
        loading
      />
      <span className="text-muted-foreground text-center text-xs">loading</span>
    </div>
  </div>
);

export default ElectricWaveformStates;
