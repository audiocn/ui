import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

import { GitHubStarsLink } from "@/components/docs/github-stars-link";
import { ThemePicker } from "@/components/docs/theme-picker";
import { siteConfig } from "@/lib/site";

export const baseOptions = (): BaseLayoutProps => ({
  links: [
    { active: "nested-url", text: "Docs", url: "/docs" },
    { active: "nested-url", text: "Components", url: "/docs/components" },
    { active: "nested-url", text: "Blocks", url: "/docs/blocks" },
    { children: <GitHubStarsLink />, secondary: true, type: "custom" },
    { children: <ThemePicker />, secondary: true, type: "custom" },
  ],
  nav: {
    title: (
      <span className="font-heading font-semibold tracking-tight">
        {siteConfig.name}
      </span>
    ),
  },
});
