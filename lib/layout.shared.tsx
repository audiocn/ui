import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

import { BrandLogo } from "@/components/brand-logo";
import { GitHubStarsLink } from "@/components/docs/github-stars-link";
import { ThemePicker } from "@/components/docs/theme-picker";
import { siteConfig } from "@/lib/site";

interface BaseOptionsConfig {
  /** Show the theme picker. The home page has its own swatches. Default true. */
  themePicker?: boolean;
}

export const baseOptions = ({
  themePicker = true,
}: BaseOptionsConfig = {}): BaseLayoutProps => ({
  links: [
    { active: "nested-url", text: "Docs", url: "/docs" },
    { active: "nested-url", text: "Components", url: "/docs/components" },
    { active: "nested-url", text: "Blocks", url: "/docs/blocks" },
    { children: <GitHubStarsLink />, secondary: true, type: "custom" },
    ...(themePicker
      ? [
          {
            children: <ThemePicker />,
            secondary: true,
            type: "custom",
          } as const,
        ]
      : []),
  ],
  nav: {
    title: (
      <span className="font-heading inline-flex items-center gap-1 font-semibold tracking-tight">
        <BrandLogo />
        {siteConfig.name}
      </span>
    ),
  },
});
