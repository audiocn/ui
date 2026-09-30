import { ServerCodeBlock } from "fumadocs-ui/components/codeblock.rsc";

import { readSource } from "@/lib/docs/read-source";

interface ComponentSourceProps {
  path: string;
  title?: string;
}

export const ComponentSource = async ({
  path,
  title,
}: ComponentSourceProps) => {
  const code = await readSource(path);
  const lang = path.endsWith(".css") ? "css" : "tsx";

  return (
    <div className="[&_pre]:max-h-[28rem]">
      <ServerCodeBlock
        code={code}
        codeblock={{ title: title ?? path }}
        lang={lang}
      />
    </div>
  );
};
