import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
} from "fumadocs-ui/layouts/docs/page";
import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { getPageMetadata } from "@/lib/social-metadata";
import { source } from "@/lib/source";
import { getMDXComponents } from "@/mdx-components";

interface PageProps {
  params: Promise<{ slug?: string[] }>;
}

const Page = async ({ params }: PageProps) => {
  const { slug } = await params;
  const page = source.getPage(slug);

  if (!page) {
    notFound();
  }

  const MdxContent = page.data.body;

  return (
    <DocsPage full={page.data.full} toc={page.data.toc}>
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription>{page.data.description}</DocsDescription>
      <DocsBody>
        <MdxContent components={getMDXComponents()} />
      </DocsBody>
    </DocsPage>
  );
};

export const generateStaticParams = () => source.generateParams();

export const generateMetadata = async ({
  params,
}: PageProps): Promise<Metadata> => {
  const { slug } = await params;
  const page = source.getPage(slug);

  if (!page) {
    notFound();
  }

  const isReactExample =
    page.url.startsWith("/docs/components/") ||
    page.url.startsWith("/docs/blocks/");
  const defaultTitle = isReactExample
    ? `${page.data.title} for React`
    : page.data.title;

  return getPageMetadata({
    description: page.data.seoDescription ?? page.data.description,
    pathname: page.url,
    title: page.data.seoTitle ?? defaultTitle,
  });
};

export default Page;
