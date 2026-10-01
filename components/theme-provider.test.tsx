import { act, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ThemeProvider } from "@/components/theme-provider";

describe("ThemeProvider", () => {
  beforeEach(() => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn((query: string) => ({
        addEventListener: vi.fn(),
        addListener: vi.fn(),
        matches: false,
        media: query,
        removeEventListener: vi.fn(),
        removeListener: vi.fn(),
      }))
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("toggles light and dark with D, subscribing once", () => {
    const addListener = vi.spyOn(window, "addEventListener");
    render(
      <ThemeProvider>
        <span />
      </ThemeProvider>
    );
    const root = document.documentElement;
    act(() => {
      fireEvent.keyDown(window, { key: "d" });
    });
    const first = root.classList.contains("dark");
    act(() => {
      fireEvent.keyDown(window, { key: "d" });
    });
    expect(root.classList.contains("dark")).toBe(!first);

    const keydowns = addListener.mock.calls.filter(
      ([type]) => type === "keydown"
    );
    expect(keydowns).toHaveLength(1);
  });
});
