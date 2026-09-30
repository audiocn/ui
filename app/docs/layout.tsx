import { DocsLayout } from "fumadocs-ui/layouts/docs";

import { baseOptions } from "@/lib/layout.shared";
import { source } from "@/lib/source";

const Layout = ({ children }: { children: React.ReactNode }) => (
  <DocsLayout tree={source.getPageTree()} {...baseOptions()}>
    {children}
  </DocsLayout>
);

export default Layout;
