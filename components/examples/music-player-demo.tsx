"use client";

import { MusicPlayer } from "@/components/blocks/music-player/music-player";
import { useDemoTracks } from "@/lib/docs/use-demo-audio";

const MusicPlayerDemo = () => {
  const tracks = useDemoTracks();
  if (tracks.length === 0) {
    return <p className="text-muted-foreground text-sm">Preparing tracks{"…"}</p>;
  }
  return <MusicPlayer className="w-full max-w-md" tracks={tracks} />;
};

export default MusicPlayerDemo;
