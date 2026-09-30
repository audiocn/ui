"use client";

import { mergeProps } from "@base-ui/react/merge-props";
import { useRender } from "@base-ui/react/use-render";
import { cva } from "class-variance-authority";
import type { VariantProps } from "class-variance-authority";
import { createContext, useContext, useMemo } from "react";
import type { ComponentProps, KeyboardEvent } from "react";

import { cn } from "@/lib/utils";

const ITEM_SELECTOR = "[data-slot='track-list-item']:not([data-disabled])";

interface TrackListItemContextValue {
  active: boolean;
  playing: boolean;
}

const TrackListItemContext = createContext<TrackListItemContextValue>({
  active: false,
  playing: false,
});

const trackListVariants = cva("group/track-list flex w-full flex-col", {
  defaultVariants: { size: "default", variant: "default" },
  variants: {
    size: {
      default: "gap-0.5 [--track-row-height:3rem]",
      lg: "gap-1 [--track-row-height:3.5rem]",
      sm: "gap-0 [--track-row-height:2.25rem]",
    },
    variant: {
      default: "",
      outline: "rounded-xl border p-1",
    },
  },
});

export interface TrackListProps
  extends ComponentProps<"ul">, VariantProps<typeof trackListVariants> {}

const moveFocus = (event: KeyboardEvent<HTMLUListElement>) => {
  const items = [
    ...event.currentTarget.querySelectorAll<HTMLElement>(ITEM_SELECTOR),
  ];
  const current = (event.target as HTMLElement).closest<HTMLElement>(
    ITEM_SELECTOR
  );
  if (!current || current !== event.target) {
    return;
  }
  const index = items.indexOf(current);
  const targets: Record<string, number> = {
    ArrowDown: index + 1,
    ArrowUp: index - 1,
    End: items.length - 1,
    Home: 0,
  };
  const targetIndex = targets[event.key];
  const next = targetIndex === undefined ? undefined : items[targetIndex];
  if (next) {
    event.preventDefault();
    next.focus();
  }
};

export const TrackList = ({
  size = "default",
  variant = "default",
  className,
  onKeyDown,
  ...props
}: TrackListProps) => (
  <ul
    className={cn(trackListVariants({ size, variant }), className)}
    data-size={size}
    data-slot="track-list"
    onKeyDown={(event) => {
      onKeyDown?.(event);
      if (!event.defaultPrevented) {
        moveFocus(event);
      }
    }}
    {...props}
  />
);

export interface TrackListItemProps extends useRender.ComponentProps<"li"> {
  /** The current track. */
  active?: boolean;
  /** The current track, playing. */
  playing?: boolean;
  disabled?: boolean;
  /** Click, Enter or Space. */
  onSelect?: () => void;
}

export const TrackListItem = ({
  active = false,
  playing = false,
  disabled = false,
  onSelect,
  render,
  className,
  children,
  ...props
}: TrackListItemProps) => {
  const contextValue = useMemo(() => ({ active, playing }), [active, playing]);

  return useRender({
    defaultTagName: "li",
    props: mergeProps<"li">(
      {
        "aria-current": active ? "true" : undefined,
        "aria-disabled": disabled || undefined,
        children: (
          <TrackListItemContext.Provider value={contextValue}>
            {children}
          </TrackListItemContext.Provider>
        ),
        className: cn(
          "group/track-list-item hover:bg-muted/60 focus-visible:ring-ring/30 data-active:bg-muted relative flex min-h-(--track-row-height) cursor-default items-center gap-3 rounded-lg px-2 text-sm transition-colors outline-none focus-visible:ring-3 data-disabled:opacity-50",
          className
        ),
        onClick: (event) => {
          const interactive = (event.target as HTMLElement).closest(
            "button, a, input"
          );
          if (
            !disabled &&
            (!interactive || interactive === event.currentTarget)
          ) {
            onSelect?.();
          }
        },
        onKeyDown: (event) => {
          if (event.target !== event.currentTarget || disabled) {
            return;
          }
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onSelect?.();
          }
        },
        tabIndex: disabled ? -1 : 0,
      },
      props
    ),
    render,
    state: { active, disabled, playing, slot: "track-list-item" },
  });
};

export const TrackListItemIndex = ({
  className,
  children,
  ...props
}: ComponentProps<"span">) => {
  const { playing } = useContext(TrackListItemContext);
  return (
    <span
      className={cn(
        "text-muted-foreground group-data-active/track-list-item:text-primary flex w-5 shrink-0 items-center justify-center font-mono text-xs tabular-nums",
        className
      )}
      data-slot="track-list-item-index"
      {...props}
    >
      {playing ? (
        <span
          aria-label="Playing"
          className="flex h-3 items-end gap-px"
          role="img"
        >
          <span className="h-3 w-0.5 animate-pulse rounded-full bg-current motion-reduce:animate-none" />
          <span className="h-2 w-0.5 animate-pulse rounded-full bg-current [animation-delay:200ms] motion-reduce:animate-none" />
          <span className="h-2.5 w-0.5 animate-pulse rounded-full bg-current [animation-delay:400ms] motion-reduce:animate-none" />
        </span>
      ) : (
        children
      )}
    </span>
  );
};

export const TrackListItemArtwork = ({
  className,
  alt = "",
  ...props
}: ComponentProps<"img">) => (
  <img
    alt={alt}
    className={cn(
      "bg-muted size-9 shrink-0 rounded-md object-cover",
      className
    )}
    data-slot="track-list-item-artwork"
    {...props}
  />
);

export const TrackListItemContent = ({
  className,
  ...props
}: ComponentProps<"div">) => (
  <div
    className={cn("flex min-w-0 flex-1 flex-col", className)}
    data-slot="track-list-item-content"
    {...props}
  />
);

export const TrackListItemTitle = ({
  className,
  ...props
}: ComponentProps<"span">) => (
  <span
    className={cn(
      "group-data-active/track-list-item:text-primary truncate font-medium",
      className
    )}
    data-slot="track-list-item-title"
    {...props}
  />
);

export const TrackListItemDescription = ({
  className,
  ...props
}: ComponentProps<"span">) => (
  <span
    className={cn("text-muted-foreground truncate text-xs", className)}
    data-slot="track-list-item-description"
    {...props}
  />
);

export const TrackListItemDuration = ({
  className,
  ...props
}: ComponentProps<"span">) => (
  <span
    className={cn(
      "text-muted-foreground shrink-0 font-mono text-xs tabular-nums",
      className
    )}
    data-slot="track-list-item-duration"
    {...props}
  />
);

export const TrackListItemActions = ({
  className,
  ...props
}: ComponentProps<"div">) => (
  <div
    className={cn("flex shrink-0 items-center gap-1", className)}
    data-slot="track-list-item-actions"
    {...props}
  />
);

export { trackListVariants };
