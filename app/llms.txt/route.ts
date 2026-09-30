import { siteConfig } from "@/lib/site";
import { source } from "@/lib/source";

export const revalidate = false;

export const GET = () => {
  const lines = source
    .getPages()
    .map((page) => `- [${page.data.title}](${siteConfig.url}${page.url}): ${page.data.description ?? ""}`);

  const body = [
    `# ${siteConfig.name}`,
    "",
    `> ${siteConfig.description}`,
    "",
    `Install components with the shadcn CLI after adding "${siteConfig.registryNamespace}": "${siteConfig.registryUrl}" to the registries in components.json.`,
    "",
    "## Docs",
    "",
    ...lines,
    "",
  ].join("\n");

  return new Response(body, { headers: { "Content-Type": "text/plain; charset=utf-8" } });
};
