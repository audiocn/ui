import type { Code, Parent, Root, RootContent } from "mdast";

/** The MDX node that renders `<InstallCommand command="..." />`. */
interface InstallCommandNode {
  attributes: { name: string; type: "mdxJsxAttribute"; value: string }[];
  children: [];
  name: "InstallCommand";
  type: "mdxJsxFlowElement";
}

const isNpmCode = (node: RootContent): node is Code =>
  node.type === "code" && node.lang === "npm";

const toInstallCommand = (node: Code): InstallCommandNode => ({
  attributes: [
    { name: "command", type: "mdxJsxAttribute", value: node.value.trim() },
  ],
  children: [],
  name: "InstallCommand",
  type: "mdxJsxFlowElement",
});

const replaceNpmCode = (parent: Parent) => {
  parent.children = parent.children.map((child) => {
    if (isNpmCode(child)) {
      return toInstallCommand(child) as unknown as RootContent;
    }
    if ("children" in child) {
      replaceNpmCode(child);
    }
    return child;
  });
};

/**
 * Renders ```npm code blocks as `<InstallCommand />`, which shows the
 * command for every package manager. Replaces fumadocs' own `remarkNpm`.
 */
export const remarkInstallCommand = () => (tree: Root) => {
  replaceNpmCode(tree);
};
