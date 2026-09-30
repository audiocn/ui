import { ServerCodeBlock } from "fumadocs-ui/components/codeblock.rsc";

import { ComponentPreviewTabs } from "@/components/docs/component-preview-tabs";
import { ExampleRenderer } from "@/components/docs/example-renderer";
import { codeThemes } from "@/lib/docs/code-themes";
import { readSource } from "@/lib/docs/read-source";

interface ComponentPreviewProps {
  align?: "center" | "start" | "end";
  className?: string;
  name: string;
}

export const ComponentPreview = async ({
  align,
  className,
  name,
}: ComponentPreviewProps) => {
  const code = await readSource(`components/examples/${name}.tsx`);

  return (
    <ComponentPreviewTabs
      align={align}
      className={className}
      code={<ServerCodeBlock code={code} lang="tsx" themes={codeThemes} />}
      preview={<ExampleRenderer name={name} />}
    />
  );
};
