"use client";

import { examples } from "@/components/docs/example-registry";

interface ExampleRendererProps {
  name: string;
}

export const ExampleRenderer = ({ name }: ExampleRendererProps) => {
  const Example = examples[name];

  if (!Example) {
    return (
      <p className="text-muted-foreground text-sm">
        Example <code>{name}</code> not found.
      </p>
    );
  }

  return <Example />;
};
