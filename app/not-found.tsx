import Link from "next/link";

import { PageState } from "@/components/docs/page-state";
import { buttonVariants } from "@/components/ui/button";

const NotFound = () => (
  <PageState
    description="This page doesn't exist. Browse the documentation to find components, hooks and blocks."
    title="Page not found"
  >
    <Link className={buttonVariants()} href="/docs">
      Browse documentation
    </Link>
    <Link className={buttonVariants({ variant: "outline" })} href="/">
      Go home
    </Link>
  </PageState>
);

export default NotFound;
