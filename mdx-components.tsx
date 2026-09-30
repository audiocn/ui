import { Callout } from "fumadocs-ui/components/callout";
import { Step, Steps } from "fumadocs-ui/components/steps";
import { Tab, Tabs } from "fumadocs-ui/components/tabs";
import { TypeTable } from "fumadocs-ui/components/type-table";
import defaultMdxComponents from "fumadocs-ui/mdx";
import type { MDXComponents } from "mdx/types";

import { ComponentPreview } from "@/components/docs/component-preview";
import { ComponentSource } from "@/components/docs/component-source";
import { PropsTable } from "@/components/docs/props-table";

export const getMDXComponents = (
  components?: MDXComponents
): MDXComponents => ({
  ...defaultMdxComponents,
  Callout,
  ComponentPreview,
  ComponentSource,
  PropsTable,
  Step,
  Steps,
  Tab,
  Tabs,
  TypeTable,
  ...components,
});
