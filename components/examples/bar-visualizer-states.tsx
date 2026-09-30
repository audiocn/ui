import { BarVisualizer } from "@/components/ui/bar-visualizer";

const BarVisualizerStates = () => (
  <div className="grid w-full max-w-md gap-6 sm:grid-cols-3">
    <div className="grid gap-2">
      <BarVisualizer aria-label="Idle pulse" barCount={9} className="h-16" idle="pulse" />
      <span className="text-center text-muted-foreground text-xs">idle pulse</span>
    </div>
    <div className="grid gap-2">
      <BarVisualizer aria-label="Idle wave" barCount={9} className="h-16" idle="wave" />
      <span className="text-center text-muted-foreground text-xs">idle wave</span>
    </div>
    <div className="grid gap-2">
      <BarVisualizer aria-label="Connecting" barCount={9} className="h-16" loading />
      <span className="text-center text-muted-foreground text-xs">loading</span>
    </div>
  </div>
);

export default BarVisualizerStates;
