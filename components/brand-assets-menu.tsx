"use client";

// Adapted from @ncdai/brand-assets-menu (MIT). See THIRD_PARTY_NOTICES.md.
import {
  DownloadSimpleIcon,
  FilePngIcon,
  FileSvgIcon,
  TextTIcon,
} from "@phosphor-icons/react";
import type { ReactElement } from "react";
import { useRef } from "react";
import { toast } from "sonner";

import { BrandLogo } from "@/components/brand-logo";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import { logomarkSVG, logotypeSVG } from "@/lib/brand-assets.json";

export const BrandAssetsMenu = ({ children }: { children: ReactElement }) => {
  const touchMenuOpened = useRef(false);
  const { copy } = useCopyToClipboard({
    onCopyError: () =>
      toast.error("Could not copy. Download the brand assets instead."),
    onCopySuccess: () => toast.success("Copied as SVG"),
  });

  return (
    <ContextMenu
      onOpenChange={(open, details) => {
        if (open && details.event.type.startsWith("touch")) {
          touchMenuOpened.current = true;
        }
      }}
    >
      <ContextMenuTrigger
        onKeyDown={(event) => {
          if (
            event.key === "ContextMenu" ||
            (event.key === "F10" && event.shiftKey)
          ) {
            event.preventDefault();
            const bounds = event.currentTarget.getBoundingClientRect();
            // Use the primitive's normal context-menu positioning and focus flow.
            event.currentTarget.dispatchEvent(
              new MouseEvent("contextmenu", {
                bubbles: true,
                cancelable: true,
                clientX: bounds.left + bounds.width / 2,
                clientY: bounds.bottom,
              })
            );
          }
        }}
        onTouchCancel={() => {
          touchMenuOpened.current = false;
        }}
        onTouchEnd={(event) => {
          if (touchMenuOpened.current) {
            // Suppress the link's synthetic click after opening by long press.
            event.preventDefault();
            touchMenuOpened.current = false;
          }
        }}
        render={children}
      />
      <ContextMenuContent aria-label="Brand assets" className="w-64">
        <ContextMenuGroup>
          <ContextMenuLabel>audiocn</ContextMenuLabel>
          <ContextMenuItem onClick={async () => await copy(logomarkSVG)}>
            <BrandLogo className="size-4" size={16} />
            Copy logo as SVG
          </ContextMenuItem>
          <ContextMenuItem onClick={async () => await copy(logotypeSVG)}>
            <TextTIcon />
            Copy wordmark as SVG
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem
            render={
              <a
                aria-label="Download logo SVG"
                download="audiocn-logo.svg"
                href="/brand/logo.svg"
              />
            }
          >
            <FileSvgIcon />
            Download logo SVG
          </ContextMenuItem>
          <ContextMenuItem
            render={
              <a
                aria-label="Download logo PNG"
                download="audiocn-logo.png"
                href="/brand/logo.png"
              />
            }
          >
            <FilePngIcon />
            Download logo PNG
          </ContextMenuItem>
          <ContextMenuItem
            render={
              <a
                aria-label="Download brand assets"
                download="audiocn-brand-assets.zip"
                href="/brand/audiocn-brand-assets.zip"
              />
            }
          >
            <DownloadSimpleIcon />
            Download brand assets
          </ContextMenuItem>
        </ContextMenuGroup>
      </ContextMenuContent>
    </ContextMenu>
  );
};
