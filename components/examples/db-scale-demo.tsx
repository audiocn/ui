import { DbScale } from "@/components/ui/db-scale";

const DbScaleDemo = () => (
  <div className="grid w-full max-w-md gap-8">
    <DbScale />
    <DbScale maxDb={6} minDb={-48} taper="audio" />
  </div>
);

export default DbScaleDemo;
