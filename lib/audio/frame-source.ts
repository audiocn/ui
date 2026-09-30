import type { FrameSource } from "@/lib/audio/types";

export interface FrameEmitter<T> extends FrameSource<T> {
  /** Sends a frame to every subscriber. */
  emit: (frame: T) => void;
  /** The last frame emitted, if any. */
  readonly latest: T | undefined;
}

/**
 * Creates a frame source you push to yourself: from a WebSocket, a worker, a
 * native bridge or a test.
 */
export const createFrameEmitter = <T>(): FrameEmitter<T> => {
  const subscribers = new Set<(frame: T) => void>();
  let latest: T | undefined;

  return {
    emit: (frame) => {
      latest = frame;
      for (const subscriber of subscribers) {
        subscriber(frame);
      }
    },
    get latest() {
      return latest;
    },
    subscribe: (callback) => {
      subscribers.add(callback);
      return () => {
        subscribers.delete(callback);
      };
    },
  };
};
