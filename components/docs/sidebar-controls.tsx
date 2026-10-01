import { ThemeSwitch } from "fumadocs-ui/layouts/shared/slots/theme-switch";

import { GitHubStarsLink } from "@/components/docs/github-stars-link";
import { ThemePicker } from "@/components/docs/theme-picker";

/** Docs sidebar footer: GitHub and the colour theme on the left, light/dark on the right. */
export const SidebarControls = () => (
  <div className="flex flex-1 items-center gap-1">
    <GitHubStarsLink />
    <ThemePicker />
    <ThemeSwitch className="ms-auto rounded-none border-y-0 border-e-0 px-1 py-0 *:rounded-md" />
  </div>
);
