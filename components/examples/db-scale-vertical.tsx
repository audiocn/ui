import { DbScale, DbScaleTick } from "@/components/ui/db-scale";

const DbScaleVertical = () => (
  <div className="flex h-56 gap-10">
    <DbScale orientation="vertical" />
    <DbScale maxDb={12} minDb={-60} orientation="vertical" side="start">
      <DbScaleTick value={12} />
      <DbScaleTick value={0}>Unity</DbScaleTick>
      <DbScaleTick major={false} value={-10} />
      <DbScaleTick value={-20} />
      <DbScaleTick major={false} value={-30} />
      <DbScaleTick value={-60}>−∞</DbScaleTick>
    </DbScale>
  </div>
);

export default DbScaleVertical;
