import {
  Mixer,
  MixerChannels,
  MixerEmpty,
  MixerHeader,
  MixerTitle,
} from "@/components/ui/mixer";

const MixerEmptyDemo = () => (
  <Mixer className="w-full max-w-md">
    <MixerHeader>
      <MixerTitle>Audio mixer</MixerTitle>
    </MixerHeader>
    <MixerChannels />
    <MixerEmpty>
      No audio sources yet. Add a microphone to get started.
    </MixerEmpty>
  </Mixer>
);

export default MixerEmptyDemo;
