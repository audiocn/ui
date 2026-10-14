import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

import {
  DocsBrandNavTitle,
  HomeBrandNavTitle,
} from "@/components/brand-nav-title";
import { GitHubStarsLink } from "@/components/docs/github-stars-link";
import { LiveInputSwitch } from "@/components/docs/live-input";
import { SidebarControls } from "@/components/docs/sidebar-controls";

interface BaseOptionsConfig {
  /**
   * Docs layout: drop the Docs link (the sidebar is the docs) and move GitHub
   * and the theme picker into the sidebar footer. Default false.
   */
  docs?: boolean;
}

export const baseOptions = ({
  docs = false,
}: BaseOptionsConfig = {}): BaseLayoutProps => ({
  links: [
    ...(docs
      ? []
      : [{ active: "nested-url", text: "Docs", url: "/docs" } as const]),
    { active: "nested-url", text: "Components", url: "/docs/components" },
    { active: "nested-url", text: "Blocks", url: "/docs/blocks" },
    ...(docs
      ? []
      : [
          {
            children: <LiveInputSwitch />,
            secondary: true,
            type: "custom",
          } as const,
          {
            children: <GitHubStarsLink />,
            secondary: true,
            type: "custom",
          } as const,
        ]),
  ],
  slots: { navTitle: docs ? DocsBrandNavTitle : HomeBrandNavTitle },
  ...(docs && { themeSwitch: { component: <SidebarControls /> } }),
});
