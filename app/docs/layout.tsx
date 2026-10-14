import { DocsLayout } from "fumadocs-ui/layouts/docs";

import { LiveInputSwitch } from "@/components/docs/live-input";
import { baseOptions } from "@/lib/layout.shared";
import { source } from "@/lib/source";

const Layout = ({ children }: { children: React.ReactNode }) => (
  <DocsLayout
    tree={source.getPageTree()}
    {...baseOptions({ docs: true })}
    sidebar={{
      // Its own row under GitHub and the theme controls, which fill theirs.
      footer: (
        <LiveInputSwitch
          key="live-input"
          className="mt-2 justify-between px-2.5"
        />
      ),
    }}
  >
    {children}
  </DocsLayout>
);

export default Layout;
