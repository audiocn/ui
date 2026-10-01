import { expect, test } from "@playwright/test";

import { siteConfig } from "../lib/site";

const cases = [
  { pathname: "/", title: siteConfig.title },
  { pathname: "/contributors", title: "Contributors — audiocn" },
  {
    pathname: "/docs/components/level-meter",
    title: "Level Meter for React — audiocn",
  },
  {
    pathname: "/docs/components",
    title: "Audio components for React — audiocn",
  },
  {
    pathname: "/docs/blocks/music-player",
    title: "Music Player for React — audiocn",
  },
  { pathname: "/docs/hooks/use-level", title: "useLevel — audiocn" },
];

for (const { pathname, title } of cases) {
  test(`${pathname} has consistent search and social metadata`, async ({
    page,
  }) => {
    await page.goto(pathname);
    await expect(page).toHaveTitle(title);
    const canonical =
      pathname === "/"
        ? siteConfig.url
        : new URL(pathname, siteConfig.url).href;
    await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
      "href",
      canonical
    );
    await expect(page.locator('meta[property="og:url"]')).toHaveAttribute(
      "content",
      canonical
    );
    await Promise.all(
      ['meta[property="og:title"]', 'meta[name="twitter:title"]'].map(
        (selector) =>
          expect(page.locator(selector)).toHaveAttribute("content", title)
      )
    );
    const description = await page
      .locator('meta[name="description"]')
      .getAttribute("content");
    expect(description?.length).toBeGreaterThan(20);
    await Promise.all(
      [
        'meta[property="og:description"]',
        'meta[name="twitter:description"]',
      ].map((selector) =>
        expect(page.locator(selector)).toHaveAttribute(
          "content",
          description ?? ""
        )
      )
    );
  });
}

test("SEO overrides preserve the visible documentation heading", async ({
  page,
}) => {
  await page.goto("/docs/components");
  await expect(
    page.getByRole("heading", { exact: true, name: "Components" })
  ).toBeVisible();
  await expect(page.locator('meta[name="description"]')).toHaveAttribute(
    "content",
    /Browse React audio components/u
  );
});
