"use client";

import { NumberField as NumberFieldPrimitive } from "@base-ui/react/number-field";
import {
  createContext,
  useContext,
  useId,
  useLayoutEffect,
  useMemo,
  useState,
} from "react";
import type { ComponentProps } from "react";

import { useLcdDisplayRowLabelId } from "@/components/ui/lcd-display";
import { cn } from "@/lib/utils";

interface LcdNumberFieldContextValue {
  /** The unit's id while one is rendered, so the input can point at it. */
  unitId: string | undefined;
  setUnitId: (id: string | undefined) => void;
}

const LcdNumberFieldContext = createContext<LcdNumberFieldContextValue>({
  setUnitId: () => {
    // Outside a field there is no input to describe.
  },
  unitId: undefined,
});

/** A 5 × 3 pixel chevron, pointing up or down. */
const PixelChevron = ({ direction }: { direction: "up" | "down" }) => (
  <svg
    aria-hidden
    className={cn("h-1.5 w-2.5 shrink-0", direction === "up" && "rotate-180")}
    fill="currentColor"
    shapeRendering="crispEdges"
    viewBox="0 0 5 3"
  >
    <path d="M0 0h1v1H0zM4 0h1v1H4zM1 1h1v1H1zM3 1h1v1H3zM2 2h1v1H2z" />
  </svg>
);

/** A 7 × 3 pixel double arrow, shown in place of the pointer while scrubbing. */
const PixelScrubArrows = () => (
  <svg
    aria-hidden
    className="h-1.5 w-3.5"
    fill="currentColor"
    shapeRendering="crispEdges"
    viewBox="0 0 7 3"
  >
    <path d="M1 0h1v1H1zM0 1h7v1H0zM5 0h1v1H5zM1 2h1v1H1zM5 2h1v1H5z" />
  </svg>
);

/**
 * Drag inside it to change the value; Shift for large steps, Alt for small
 * ones. Wrap the row's label in it, like a hardware encoder's legend.
 */
export const LcdNumberFieldScrubArea = ({
  direction = "horizontal",
  className,
  children,
  ...props
}: NumberFieldPrimitive.ScrubArea.Props) => (
  <NumberFieldPrimitive.ScrubArea
    className={cn(
      "inline-flex touch-none items-center select-none data-disabled:cursor-default",
      direction === "vertical" ? "cursor-ns-resize" : "cursor-ew-resize",
      className
    )}
    data-slot="lcd-number-field-scrub-area"
    direction={direction}
    {...props}
  >
    {children}
    <NumberFieldPrimitive.ScrubAreaCursor
      className="text-(--lcd-display-lit) drop-shadow-[0_0_3px_var(--lcd-display-glow)]"
      data-slot="lcd-number-field-scrub-cursor"
    >
      <span className={cn("block", direction === "vertical" && "rotate-90")}>
        <PixelScrubArrows />
      </span>
    </NumberFieldPrimitive.ScrubAreaCursor>
  </NumberFieldPrimitive.ScrubArea>
);

/** The value, its unit and its steppers, on one line. */
export const LcdNumberFieldGroup = ({
  className,
  ...props
}: NumberFieldPrimitive.Group.Props) => (
  <NumberFieldPrimitive.Group
    className={cn("inline-flex items-center gap-2", className)}
    data-slot="lcd-number-field-group"
    {...props}
  />
);

/**
 * The number in lit ink. Selecting text inverts it, like an LCD cursor. Named
 * by the row's label and described by the unit.
 */
export const LcdNumberFieldInput = ({
  className,
  "aria-labelledby": ariaLabelledBy,
  "aria-describedby": ariaDescribedBy,
  ...props
}: NumberFieldPrimitive.Input.Props) => {
  const rowLabelId = useLcdDisplayRowLabelId();
  const { unitId } = useContext(LcdNumberFieldContext);
  const describedBy =
    [ariaDescribedBy, unitId].filter(Boolean).join(" ") || undefined;
  return (
    <NumberFieldPrimitive.Input
      aria-describedby={describedBy}
      aria-labelledby={ariaLabelledBy ?? rowLabelId}
      className={cn(
        "-mx-1 field-sizing-content max-w-[12ch] min-w-[3ch] rounded-sm bg-transparent px-1 py-0.5 text-end text-(--lcd-display-lit) uppercase tabular-nums caret-(--lcd-display-lit) outline-none selection:bg-(--lcd-display-lit) selection:text-(--lcd-display-glass) focus-visible:ring-1 focus-visible:ring-(--lcd-display-lit) disabled:text-(--lcd-display-ink-dim) data-invalid:text-(--lcd-display-ink-dim)",
        className
      )}
      data-slot="lcd-number-field-input"
      {...props}
    />
  );
};

/** The unit after the number, in dim ink. Screen readers hear it with the value. */
export const LcdNumberFieldUnit = ({
  className,
  id,
  ...props
}: ComponentProps<"span">) => {
  const generatedId = useId();
  const unitId = id ?? generatedId;
  const { setUnitId } = useContext(LcdNumberFieldContext);
  useLayoutEffect(() => {
    setUnitId(unitId);
    return () => setUnitId(undefined);
  }, [setUnitId, unitId]);
  return (
    <span
      className={cn("text-(--lcd-display-ink-dim)", className)}
      data-slot="lcd-number-field-unit"
      id={unitId}
      {...props}
    />
  );
};

const STEPPER_BUTTON =
  "flex cursor-pointer items-center justify-center rounded-[1px] px-0.5 py-px text-(--lcd-display-ink-dim) outline-none hover:text-(--lcd-display-lit) active:text-(--lcd-display-lit) disabled:cursor-default disabled:opacity-40 disabled:hover:text-(--lcd-display-ink-dim)";

/** Steps the value up. Hold to repeat. */
export const LcdNumberFieldIncrement = ({
  className,
  children,
  ...props
}: NumberFieldPrimitive.Increment.Props) => (
  <NumberFieldPrimitive.Increment
    className={cn(STEPPER_BUTTON, className)}
    data-slot="lcd-number-field-increment"
    {...props}
  >
    {children ?? <PixelChevron direction="up" />}
  </NumberFieldPrimitive.Increment>
);

/** Steps the value down. Hold to repeat. */
export const LcdNumberFieldDecrement = ({
  className,
  children,
  ...props
}: NumberFieldPrimitive.Decrement.Props) => (
  <NumberFieldPrimitive.Decrement
    className={cn(STEPPER_BUTTON, className)}
    data-slot="lcd-number-field-decrement"
    {...props}
  >
    {children ?? <PixelChevron direction="down" />}
  </NumberFieldPrimitive.Decrement>
);

/** The increment over the decrement, as a stacked pair of pixel chevrons. */
export const LcdNumberFieldStepper = ({
  className,
  children,
  ...props
}: ComponentProps<"span">) => (
  <span
    className={cn("inline-flex flex-col gap-0.5", className)}
    data-slot="lcd-number-field-stepper"
    {...props}
  >
    {children ?? (
      <>
        <LcdNumberFieldIncrement />
        <LcdNumberFieldDecrement />
      </>
    )}
  </span>
);

export type LcdNumberFieldProps = NumberFieldPrimitive.Root.Props;

/**
 * An editable number on the LCD Display, built on Base UI's Number Field:
 * type it, step it with the arrow keys, the wheel or the pixel steppers, or
 * drag the scrub area. Its root lays out no box of its own (`display:
 * contents`), so its parts sit in the row beside the label. With no children
 * it renders the input and the steppers.
 */
export const LcdNumberField = ({
  allowWheelScrub = true,
  className,
  children,
  ...props
}: LcdNumberFieldProps) => {
  const [unitId, setUnitId] = useState<string>();
  const context = useMemo(() => ({ setUnitId, unitId }), [unitId]);
  return (
    <LcdNumberFieldContext.Provider value={context}>
      <NumberFieldPrimitive.Root
        allowWheelScrub={allowWheelScrub}
        className={cn("contents", className)}
        data-slot="lcd-number-field"
        {...props}
      >
        {children ?? (
          <LcdNumberFieldGroup>
            <LcdNumberFieldInput />
            <LcdNumberFieldStepper />
          </LcdNumberFieldGroup>
        )}
      </NumberFieldPrimitive.Root>
    </LcdNumberFieldContext.Provider>
  );
};
