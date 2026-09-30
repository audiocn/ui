import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

class ResizeObserverStub {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}

if (!("ResizeObserver" in globalThis)) {
  globalThis.ResizeObserver =
    ResizeObserverStub as unknown as typeof ResizeObserver;
}

if (!("matchMedia" in globalThis)) {
  Object.defineProperty(globalThis, "matchMedia", {
    value: (query: string) => ({
      addEventListener: vi.fn(),
      addListener: vi.fn(),
      dispatchEvent: vi.fn(),
      matches: false,
      media: query,
      onchange: null,
      removeEventListener: vi.fn(),
      removeListener: vi.fn(),
    }),
    writable: true,
  });
}
