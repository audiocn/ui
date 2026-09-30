import type { ReactNode } from "react";

import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty";

interface PageStateProps {
  title: string;
  description: string;
  children: ReactNode;
}

export const PageState = ({ title, description, children }: PageStateProps) => (
  <main className="mx-auto flex w-full max-w-xl flex-1 items-center px-4 py-16">
    <Empty>
      <EmptyHeader>
        <EmptyTitle>
          <h1>{title}</h1>
        </EmptyTitle>
        <EmptyDescription>{description}</EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <div className="flex flex-wrap justify-center gap-3">{children}</div>
      </EmptyContent>
    </Empty>
  </main>
);
