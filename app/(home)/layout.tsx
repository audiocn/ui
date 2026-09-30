import { HomeLayout } from "fumadocs-ui/layouts/home";

import { baseOptions } from "@/lib/layout.shared";

const Layout = ({ children }: { children: React.ReactNode }) => (
  <HomeLayout {...baseOptions({ themePicker: false })}>{children}</HomeLayout>
);

export default Layout;
