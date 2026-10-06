import { markdownComponents } from "@/lib/docs/markdown-components";
import { source } from "@/lib/source";

export const revalidate = false;

export const GET = async () => {
  const pages = await Promise.all(
    source.getPages().map(async (page) => {
      const text = await page.data.getText("processed", {
        components: markdownComponents,
      });
      return `# ${page.data.title} (${page.url})\n\n${text}`;
    })
  );

  return new Response(pages.join("\n\n"), {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
};
