import { siteConfig } from "@/lib/site";
import registry from "@/registry.json";

const NAMESPACE = `${siteConfig.registryNamespace}/`;

export interface RegistryFile {
  path: string;
  type: string;
}

export interface RegistryItem {
  name: string;
  type: string;
  title: string;
  description: string;
  dependencies?: string[];
  registryDependencies?: string[];
  files: RegistryFile[];
  cssVars?: Record<string, Record<string, string>>;
  categories?: string[];
}

/** What the shadcn CLI writes and installs for one item, resolved transitively. */
export interface InstallPlan {
  /** The item itself. */
  item: RegistryItem;
  /** audiocn items pulled in with it, in the order the registry declares them. */
  registryItems: RegistryItem[];
  /** shadcn/ui items pulled in with it, such as `card`. */
  shadcnItems: string[];
  /** npm packages installed with it. */
  npmDependencies: string[];
  /** CSS custom properties added to the global stylesheet, with their `--` prefix. */
  cssVariables: string[];
}

const items = registry.items as RegistryItem[];
const byName = new Map(items.map((item) => [item.name, item]));

export const getRegistryItem = (name: string): RegistryItem | undefined =>
  byName.get(name);

export const registryItemUrl = (name: string): string =>
  siteConfig.registryUrl.replace("{name}", name);

/** The file a page's component lives in, such as `components/ui/knob.tsx`. */
export const primaryFile = (item: RegistryItem): string =>
  item.files[0]?.path ?? "";

const EXTENSION = /\.[^./]+$/u;

/** The alias a consumer imports the item from, such as `@/components/ui/knob`. */
export const importPathFor = (item: RegistryItem): string =>
  `@/${primaryFile(item).replace(EXTENSION, "")}`;

const byFile = new Map<string, RegistryItem>(
  items.flatMap((item) =>
    item.files.map(
      (file) => [`@/${file.path.replace(EXTENSION, "")}`, item] as const
    )
  )
);

/** The item an import such as `@/hooks/use-demo-signal` comes from. */
export const itemForImportPath = (path: string): RegistryItem | undefined =>
  byFile.get(path);

/** Docs sections whose last path segment is a registry item name. */
const ITEM_SECTIONS = [
  "/docs/components/",
  "/docs/blocks/",
  "/docs/hooks/",
] as const;

/** The item a docs page documents, if it documents one. */
export const registryItemForPath = (
  pathname: string
): RegistryItem | undefined => {
  if (!ITEM_SECTIONS.some((section) => pathname.startsWith(section))) {
    return undefined;
  }
  return byName.get(pathname.slice(pathname.lastIndexOf("/") + 1));
};

export const resolveInstall = (item: RegistryItem): InstallPlan => {
  const registryItems: RegistryItem[] = [];
  const shadcnItems: string[] = [];
  const npmDependencies: string[] = [];
  const cssVariables: string[] = [];
  const seen = new Set<string>([item.name]);

  const addNpm = (dependency: string) => {
    if (!npmDependencies.includes(dependency)) {
      npmDependencies.push(dependency);
    }
  };

  const addCssVars = (current: RegistryItem) => {
    for (const name of Object.keys(current.cssVars?.light ?? {})) {
      const variable = `--${name}`;
      if (!cssVariables.includes(variable)) {
        cssVariables.push(variable);
      }
    }
  };

  const visit = (current: RegistryItem) => {
    for (const dependency of current.dependencies ?? []) {
      addNpm(dependency);
    }
    addCssVars(current);

    for (const dependency of current.registryDependencies ?? []) {
      if (!dependency.startsWith(NAMESPACE)) {
        if (!shadcnItems.includes(dependency)) {
          shadcnItems.push(dependency);
        }
        continue;
      }
      const name = dependency.slice(NAMESPACE.length);
      if (seen.has(name)) {
        continue;
      }
      seen.add(name);
      const next = byName.get(name);
      if (!next) {
        throw new Error(`Unknown registry dependency "${dependency}"`);
      }
      registryItems.push(next);
      visit(next);
    }
  };

  visit(item);

  return {
    cssVariables,
    item,
    npmDependencies,
    registryItems,
    shadcnItems,
  };
};
