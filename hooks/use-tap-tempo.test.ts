import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { useTapTempo } from "@/hooks/use-tap-tempo";

const tapAt = (tap: () => void, time: number) => {
  vi.spyOn(performance, "now").mockReturnValue(time);
  act(tap);
};

afterEach(() => vi.restoreAllMocks());

it("normalizes default, controlled, edited and tapped values to whole BPM", () => {
  const onValueChange = vi.fn();
  const { result, rerender } = renderHook(
    ({ value }: { value?: number }) =>
      useTapTempo({ defaultValue: 90.6, onValueChange, value }),
    { initialProps: {} }
  );
  expect(result.current.value).toBe(91);
  act(() => result.current.setValue(123.4));
  expect(result.current.value).toBe(123);
  expect(onValueChange).toHaveBeenLastCalledWith(123, { reason: "input" });
  tapAt(result.current.tap, 0);
  tapAt(result.current.tap, 487);
  expect(result.current.value).toBe(123);
  rerender({ value: 96.7 });
  expect(result.current.value).toBe(97);
});

it("keeps normalized BPM inside fractional bounds", () => {
  const { result } = renderHook(() => useTapTempo({ max: 180.5, min: 60.5 }));
  act(() => result.current.setValue(30));
  expect(result.current.value).toBe(61);
  act(() => result.current.setValue(200));
  expect(result.current.value).toBe(180);
});

it("reports every distinct edit in a batch, including returning to the original value", () => {
  const onValueChange = vi.fn();
  const { result } = renderHook(() => useTapTempo({ onValueChange }));
  act(() => {
    result.current.setValue(100);
    result.current.setValue(100);
    result.current.setValue(120);
  });
  expect(result.current.value).toBe(120);
  expect(onValueChange.mock.calls).toEqual([
    [100, { reason: "input" }],
    [120, { reason: "input" }],
  ]);
});

it("reports repeated controlled edits without requiring the parent to rerender", () => {
  const onValueChange = vi.fn();
  const { result } = renderHook(() =>
    useTapTempo({ onValueChange, value: 90 })
  );
  act(() => result.current.setValue(100));
  expect(result.current.value).toBe(90);
  act(() => result.current.setValue(100));
  expect(result.current.value).toBe(90);
  expect(onValueChange).toHaveBeenCalledTimes(2);
  expect(onValueChange).toHaveBeenLastCalledWith(100, { reason: "input" });
});

it("compares controlled edits against the authoritative value after a rejected edit", () => {
  const onValueChange = vi.fn();
  const { result } = renderHook(() =>
    useTapTempo({ onValueChange, value: 90 })
  );
  act(() => result.current.setValue(100));
  act(() => result.current.setValue(90));
  expect(result.current.value).toBe(90);
  expect(onValueChange).toHaveBeenCalledExactlyOnceWith(100, {
    reason: "input",
  });
});

it("measures high tempos without a component or provider", () => {
  const { result } = renderHook(() => useTapTempo());
  tapAt(result.current.tap, 0);
  expect(result.current.value).toBe(120);
  tapAt(result.current.tap, 150);
  expect(result.current.value).toBe(400);
  expect(result.current.tapCount).toBe(2);
});

it("counts accepted taps for feedback even when BPM does not change", () => {
  const { result } = renderHook(() => useTapTempo());
  tapAt(result.current.tap, 0);
  tapAt(result.current.tap, 40);
  expect(result.current.tapCount).toBe(1);
  tapAt(result.current.tap, 500);
  expect(result.current.value).toBe(120);
  expect(result.current.tapCount).toBe(2);
  tapAt(result.current.tap, 1000);
  expect(result.current.value).toBe(120);
  expect(result.current.tapCount).toBe(3);
});

it("resets history on edits and clamps values to the default maximum", () => {
  const { result } = renderHook(() => useTapTempo());
  tapAt(result.current.tap, 0);
  act(() => result.current.setValue(750));
  expect(result.current.value).toBe(600);
  tapAt(result.current.tap, 600);
  expect(result.current.value).toBe(600);
  tapAt(result.current.tap, 1200);
  expect(result.current.value).toBe(100);
  act(result.current.resetTaps);
  tapAt(result.current.tap, 1700);
  expect(result.current.value).toBe(100);
});
