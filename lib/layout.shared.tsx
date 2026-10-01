import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

import { BrandLogo } from "@/components/brand-logo";
import { GitHubStarsLink } from "@/components/docs/github-stars-link";
import { SidebarControls } from "@/components/docs/sidebar-controls";
import { siteConfig } from "@/lib/site";

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
            children: <GitHubStarsLink />,
            secondary: true,
            type: "custom",
          } as const,
        ]),
  ],
  nav: {
    title: (
      <span className="font-heading inline-flex items-center gap-1 font-semibold tracking-tight">
        <BrandLogo />
        {siteConfig.name}
      </span>
    ),
  },
  ...(docs && { themeSwitch: { component: <SidebarControls /> } }),
});
