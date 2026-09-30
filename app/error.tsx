"use client";

import Link from "next/link";

import { PageState } from "@/components/docs/page-state";
import { Button, buttonVariants } from "@/components/ui/button";

const ErrorPage = ({ retry }: { retry: () => void }) => (
  <PageState
    description="This page couldn't load. Try again, or return to the documentation."
    title="Something went wrong"
  >
    <Button onClick={retry}>Try again</Button>
    <Link className={buttonVariants({ variant: "outline" })} href="/docs">
      Browse documentation
    </Link>
  </PageState>
);

export default ErrorPage;
