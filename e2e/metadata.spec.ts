import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { siteConfig } from "../lib/site";
import socialImages from "../lib/social-images.json" with { type: "json" };
import type { SocialImage } from "../lib/social-metadata";
import { publicPages } from "./routes";

const images: Record<string, SocialImage | undefined> = socialImages;

const readInitialHead = (page: Page, html: string) =>
  page.evaluate((markup) => {
    // DOMParser does not execute scripts: this inspects the crawler's response.
    const { head } = new DOMParser().parseFromString(markup, "text/html");
    return {
      canonical: head
        .querySelector('link[rel="canonical"]')
        ?.getAttribute("href"),
      meta: Object.fromEntries(
        Array.from(head.querySelectorAll("meta"), (element) => [
          element.getAttribute("property") ?? element.getAttribute("name"),
          element.getAttribute("content"),
        ])
      ),
      title: head.querySelector("title")?.textContent,
    };
  }, html);

for (const pathname of publicPages) {
  test(`${pathname} exposes its complete metadata to a social crawler`, async ({
    request,
    page,
  }) => {
    const response = await request.get(pathname, {
      headers: { "user-agent": "Twitterbot/1.0" },
    });
    expect(response.status()).toBe(200);
    const head = await readInitialHead(page, await response.text());
    const meta = (selector: string) => head.meta[selector];
    const canonical =
      pathname === "/"
        ? siteConfig.url
        : new URL(pathname, siteConfig.url).href;
    expect(head.canonical).toBe(canonical);
    expect(meta("og:url")).toBe(canonical);
    const { title } = head;
    expect(title).toContain("audiocn");
    expect(meta("og:title")).toBe(title);
    expect(meta("twitter:title")).toBe(title);
    const description = meta("description");
    expect(description?.length).toBeGreaterThan(20);
    expect(meta("og:description")).toBe(description);
    expect(meta("twitter:description")).toBe(description);
    const image = images[pathname];
    expect(image).toBeDefined();
    if (!image) {
      throw new Error(`Missing social image for ${pathname}`);
    }
    const imageUrl = new URL(image.url, siteConfig.url).href;
    expect(meta("og:image")).toBe(imageUrl);
    expect(meta("twitter:image")).toBe(imageUrl);
    expect(meta("og:image:alt")).toBe(image.alt);
    expect(meta("twitter:image:alt")).toBe(image.alt);
    expect(meta("og:image:width")).toBe("1200");
    expect(meta("og:image:height")).toBe("630");
    expect(meta("twitter:card")).toBe("summary_large_image");
    const asset = await request.get(image.url);
    expect(asset.status()).toBe(200);
    expect(asset.headers()["content-type"]).toContain("image/png");
  });
}

test("Slack sees documentation metadata in the initial HTML head", async ({
  request,
  page,
}) => {
  const response = await request.get("/docs/blocks/system-audio-mixer", {
    headers: { "user-agent": "Slackbot-LinkExpanding 1.0" },
  });
  const head = await readInitialHead(page, await response.text());
  expect(head.meta["og:image"]).toBe(
    new URL(
      images["/docs/blocks/system-audio-mixer"]?.url ?? "",
      siteConfig.url
    ).href
  );
});

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

test("each initial component card is distinct and supplies alt text on both networks", async ({
  page,
  request,
}) => {
  const imageUrls = new Set<string>();
  for (const slug of [
    "mixer",
    "level-meter",
    "knob",
    "waveform",
    "electric-waveform",
  ]) {
    // Navigations use one page; they must complete in order.
    // eslint-disable-next-line no-await-in-loop
    await page.goto(`/docs/components/${slug}`);
    // eslint-disable-next-line no-await-in-loop
    const imageUrl = await page
      .locator('meta[property="og:image"]')
      .getAttribute("content");
    expect(imageUrl).toMatch(new RegExp(`/og/${slug}-[a-f0-9]+\\.png$`, "u"));
    imageUrls.add(imageUrl ?? "");
    // eslint-disable-next-line no-await-in-loop
    await expect(page.locator('meta[name="twitter:image"]')).toHaveAttribute(
      "content",
      imageUrl ?? ""
    );
    // eslint-disable-next-line no-await-in-loop
    const alt = await page
      .locator('meta[property="og:image:alt"]')
      .getAttribute("content");
    expect(alt?.length).toBeGreaterThan(30);
    // eslint-disable-next-line no-await-in-loop
    await expect(
      page.locator('meta[name="twitter:image:alt"]')
    ).toHaveAttribute("content", alt ?? "");
    // eslint-disable-next-line no-await-in-loop
    const image = await request.get(new URL(imageUrl ?? "").pathname);
    expect(image.status()).toBe(200);
    expect(image.headers()["content-type"]).toContain("image/png");
  }
  expect(imageUrls.size).toBe(5);
});

test("the capture route is unavailable in production and absent from the sitemap", async ({
  request,
}) => {
  const response = await request.get("/social-preview/home");
  expect(response.status()).toBe(404);
  const catalog = await request.get("/api/social-cards");
  expect(catalog.status()).toBe(404);
  const sitemap = await request.get("/sitemap.xml");
  expect(await sitemap.text()).not.toContain("social-preview");
  expect(await sitemap.text()).not.toContain("social-cards");
});
