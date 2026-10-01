"use client";

import {
  CheckIcon,
  CubeIcon,
  FunctionIcon,
  GithubLogoIcon,
  LightbulbIcon,
  MoonIcon,
  RocketIcon,
  SquaresFourIcon,
  SunIcon,
  UsersIcon,
} from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { useDocsSearch } from "fumadocs-core/search/client";
import { fetchClient } from "fumadocs-core/search/client/fetch";
import {
  SearchDialog,
  SearchDialogClose,
  SearchDialogContent,
  SearchDialogHeader,
  SearchDialogIcon,
  SearchDialogInput,
  SearchDialogList,
  SearchDialogListItem,
  SearchDialogOverlay,
} from "fumadocs-ui/components/dialog/search";
import type { SearchItemType } from "fumadocs-ui/components/dialog/search";
import type { SharedProps } from "fumadocs-ui/contexts/search";
import { useTheme } from "next-themes";
import { createContext, useContext, useMemo } from "react";
import type { CSSProperties, ReactNode } from "react";

import { useSiteTheme } from "@/components/docs/theme-picker";
import { THEMES } from "@/lib/docs/site-themes";
import type { SearchGroup } from "@/lib/search-groups";
import { siteConfig } from "@/lib/site";

const client = fetchClient();

/** Docs groups in the order the menu shows them, with their icons. */
const DOCS_GROUPS: { label: string; icon: Icon }[] = [
  { icon: RocketIcon, label: "Getting started" },
  { icon: CubeIcon, label: "Components" },
  { icon: SquaresFourIcon, label: "Blocks" },
  { icon: FunctionIcon, label: "Hooks" },
  { icon: LightbulbIcon, label: "Concepts" },
];

const SearchGroupsContext = createContext<SearchGroup[]>([]);

/** Hands the docs pages, read on the server, to the search dialog. */
export const SearchGroupsProvider = ({
  groups,
  children,
}: {
  groups: SearchGroup[];
  children: ReactNode;
}) => (
  <SearchGroupsContext.Provider value={groups}>
    {children}
  </SearchGroupsContext.Provider>
);

interface MenuItem {
  item: SearchItemType;
  content: ReactNode;
  /** Shown while searching when the query matches, for items search can't find. */
  keywords?: string;
}

interface MenuGroup {
  label: string;
  items: MenuItem[];
}

const ItemContent = ({
  icon: ItemIcon,
  children,
  trailing,
}: {
  icon: Icon;
  children: ReactNode;
  trailing?: ReactNode;
}) => (
  <span className="flex items-center gap-2.5">
    <ItemIcon aria-hidden className="text-muted-foreground size-4" />
    <span className="min-w-0 flex-1 truncate">{children}</span>
    {trailing}
  </span>
);

const pageItem = (title: string, url: string, icon: Icon): MenuItem => ({
  content: <ItemContent icon={icon}>{title}</ItemContent>,
  item: { content: title, id: `menu:${url}`, type: "page", url },
  keywords: title,
});

const LINK_GROUP: MenuGroup = {
  items: [
    pageItem("Contributors", "/contributors", UsersIcon),
    {
      content: <ItemContent icon={GithubLogoIcon}>GitHub</ItemContent>,
      item: {
        content: "GitHub",
        external: true,
        id: "links:github",
        type: "page",
        url: `https://github.com/${siteConfig.githubRepo}`,
      },
      keywords: "GitHub source repository",
    },
  ],
  label: "Links",
};

const useThemeGroup = (): MenuGroup => {
  const { resolvedTheme, setTheme } = useTheme();
  const [siteTheme, setSiteTheme] = useSiteTheme();
  const dark = resolvedTheme === "dark";

  return useMemo(
    () => ({
      items: [
        {
          content: (
            <ItemContent
              icon={dark ? SunIcon : MoonIcon}
              trailing={
                <kbd className="text-muted-foreground border-border rounded border px-1.5 font-mono text-xs">
                  D
                </kbd>
              }
            >
              {dark ? "Light mode" : "Dark mode"}
            </ItemContent>
          ),
          item: {
            id: "theme:mode",
            node: null,
            onSelect: () => setTheme(dark ? "light" : "dark"),
            type: "action",
          },
          keywords: "theme mode dark light",
        },
        ...THEMES.map((theme): MenuItem => ({
          content: (
            <span className="flex items-center gap-2.5">
              <span
                aria-hidden
                className="mx-0.5 size-3 rounded-full bg-(--swatch)"
                style={{ "--swatch": theme.swatch } as CSSProperties}
              />
              <span className="flex-1">{theme.label}</span>
              {theme.value === siteTheme && (
                <CheckIcon
                  aria-label="Current theme"
                  className="text-muted-foreground size-4"
                />
              )}
            </span>
          ),
          item: {
            id: `theme:${theme.value}`,
            node: null,
            onSelect: () => setSiteTheme(theme.value),
            type: "action",
          },
          keywords: `theme colour color ${theme.label}`,
        })),
      ],
      label: "Theme",
    }),
    [dark, setSiteTheme, setTheme, siteTheme]
  );
};

const WHITESPACE = /\s+/u;

/** Every word of the query appears in the keywords, in any order. */
const matches = (keywords: string | undefined, query: string) => {
  const words = query.toLowerCase().split(WHITESPACE).filter(Boolean);
  const text = keywords?.toLowerCase() ?? "";
  return words.length > 0 && words.every((word) => text.includes(word));
};

interface MenuContextValue {
  content: Map<string, ReactNode>;
  /** Group names, by the id of each group's first item. */
  headings: Map<string, string>;
  /** Search results show without group headings. */
  searching: boolean;
}

const MenuContext = createContext<MenuContextValue>({
  content: new Map(),
  headings: new Map(),
  searching: false,
});

const MenuListItem = ({
  item,
  onClick,
}: {
  item: SearchItemType;
  onClick: () => void;
}) => {
  const { content, headings, searching } = useContext(MenuContext);
  const heading = searching ? undefined : headings.get(item.id);
  return (
    <>
      {heading && (
        <div
          className="text-muted-foreground px-2.5 pt-3 pb-1.5 text-xs font-medium first:pt-1.5"
          role="presentation"
        >
          {heading}
        </div>
      )}
      <SearchDialogListItem item={item} onClick={onClick}>
        {content.get(item.id)}
      </SearchDialogListItem>
    </>
  );
};

const renderMenuItem = (props: {
  item: SearchItemType;
  onClick: () => void;
}) => <MenuListItem {...props} />;

/**
 * The docs search, opened with ⌘K. Before anything is typed it lists the
 * docs pages by section, links and theme switches, so there is somewhere
 * to go straight away.
 */
export const DocsSearchDialog = (props: SharedProps) => {
  const docsGroups = useContext(SearchGroupsContext);
  const themeGroup = useThemeGroup();
  const { search, setSearch, query } = useDocsSearch({ client });

  const groups = useMemo((): MenuGroup[] => {
    const docs = DOCS_GROUPS.flatMap(({ icon, label }) => {
      const group = docsGroups.find((candidate) => candidate.label === label);
      if (!group) {
        return [];
      }
      return {
        items: group.items.map(({ title, url }) => pageItem(title, url, icon)),
        label,
      };
    });
    return [...docs, LINK_GROUP, themeGroup];
  }, [docsGroups, themeGroup]);

  const menu = useMemo(() => {
    const content = new Map<string, ReactNode>();
    const headings = new Map<string, string>();
    for (const group of groups) {
      const [first] = group.items;
      if (first) {
        headings.set(first.item.id, group.label);
      }
      for (const { item, content: node } of group.items) {
        content.set(item.id, node);
      }
    }
    return { content, headings };
  }, [groups]);

  const searching = query.data !== "empty";
  const menuValue = useMemo(() => ({ ...menu, searching }), [menu, searching]);
  const items = useMemo((): SearchItemType[] => {
    if (query.data === "empty") {
      return groups.flatMap((group) => group.items.map(({ item }) => item));
    }
    // Search finds docs pages; links and theme switches are matched here.
    const extras = [LINK_GROUP, themeGroup]
      .flatMap((group) => group.items)
      .filter(({ keywords }) => matches(keywords, search))
      .map(({ item }) => item);
    return [...(query.data ?? []), ...extras];
  }, [groups, query.data, search, themeGroup]);

  return (
    <SearchDialog
      isLoading={query.isLoading}
      onSearchChange={setSearch}
      search={search}
      {...props}
    >
      <SearchDialogOverlay />
      <SearchDialogContent>
        <SearchDialogHeader>
          <SearchDialogIcon />
          <SearchDialogInput />
          <SearchDialogClose />
        </SearchDialogHeader>
        <MenuContext.Provider value={menuValue}>
          <SearchDialogList Item={renderMenuItem} items={items} />
        </MenuContext.Provider>
      </SearchDialogContent>
    </SearchDialog>
  );
};
