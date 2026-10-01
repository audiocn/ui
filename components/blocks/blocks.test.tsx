import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MicSetup } from "@/components/blocks/mic-setup/mic-setup";
import { MusicPlayer } from "@/components/blocks/music-player/music-player";
import { SystemAudioSettings } from "@/components/blocks/system-audio-settings/system-audio-settings";

const TRACKS = [{ id: "demo", src: "demo.wav", title: "Demo" }];

describe("blocks rendered twice on one page", () => {
  it("keep every DOM id unique", () => {
    render(
      <>
        <MicSetup />
        <MicSetup />
        <MusicPlayer defaultTracks={TRACKS} duckingSource={null} />
        <MusicPlayer defaultTracks={TRACKS} duckingSource={null} />
        <SystemAudioSettings />
        <SystemAudioSettings />
      </>
    );
    const ids = [...document.querySelectorAll("[id]")].map(({ id }) => id);
    expect(ids.length).toBeGreaterThan(0);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("toggle the switch of the instance whose label was clicked", () => {
    render(
      <>
        <MicSetup />
        <MicSetup />
      </>
    );
    const [, secondLabel] = screen.getAllByText("Mute microphone");
    const [first, second] = screen.getAllByRole("switch", {
      name: "Mute microphone",
    });
    fireEvent.click(secondLabel as HTMLElement);
    expect(second).toHaveAttribute("aria-checked", "true");
    expect(first).toHaveAttribute("aria-checked", "false");
  });
});
