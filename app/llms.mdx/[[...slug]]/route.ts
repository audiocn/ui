import { buildAiPrompt, buildPageMarkdown } from "@/lib/docs/ai-prompt";
import { markdownComponents } from "@/lib/docs/markdown-components";
import { registryItemForPath, resolveInstall } from "@/lib/docs/registry";
import { source } from "@/lib/source";

export const revalidate = false;

export const generateStaticParams = () => source.generateParams();

/**
 * The Markdown twin of every docs page, reached as `<page>.md` through the
 * rewrite in `next.config.ts`. Pages that document a registry item get the
 * full "Copy prompt for AI" document; the rest get their body.
 */
export const GET = async (
  _request: Request,
  { params }: { params: Promise<{ slug?: string[] }> }
) => {
  const { slug } = await params;
  const page = source.getPage(slug);

  if (!page) {
    return new Response("Not found", {
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      status: 404,
    });
  }

  const body = await page.data.getText("processed", {
    components: markdownComponents,
  });
  const item = registryItemForPath(page.url);
  const common = {
    body,
    description: page.data.description,
    pathname: page.url,
    title: page.data.title,
  };

  const markdown = item
    ? buildAiPrompt({ ...common, install: resolveInstall(item) })
    : buildPageMarkdown(common);

  return new Response(markdown, {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
};
