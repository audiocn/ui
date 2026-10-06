import { expect, test } from "@playwright/test";

const PAGE = "/docs/components/bar-visualizer";
const INSTALL = "npx shadcn@latest add @audiocn/bar-visualizer";

test("a component page serves a Markdown twin written for agents", async ({
  request,
}) => {
  const response = await request.get(`${PAGE}.md`);
  expect(response.status()).toBe(200);
  expect(response.headers()["content-type"]).toContain("text/markdown");

  const markdown = await response.text();
  expect(markdown).toContain(
    "# Add Bar Visualizer from audiocn to this project"
  );
  expect(markdown).toContain(INSTALL);
  // The example, inlined from the file the page previews.
  expect(markdown).toContain("components/examples/bar-visualizer-demo.tsx");
  expect(markdown).toContain("| Prop | Type | Default | Description |");
  expect(markdown).toContain("@/components/ui/bar-visualizer");
  // Every MDX component has a Markdown form, so none leak as JSX.
  expect(markdown).not.toMatch(/<(?:ComponentPreview|PropsTable|Callout)\b/u);
});

test("a page with no registry item still has a Markdown twin", async ({
  request,
}) => {
  const response = await request.get("/docs/concepts/decibels.md");
  expect(response.status()).toBe(200);
  const markdown = await response.text();
  expect(markdown).toContain("# Decibels and levels");
  expect(markdown).not.toContain("npx shadcn@latest add");
});

test("llms-full.txt renders the MDX components rather than their tags", async ({
  request,
}) => {
  const response = await request.get("/llms-full.txt");
  const markdown = await response.text();
  expect(markdown).toContain(INSTALL);
  expect(markdown).not.toMatch(/<(?:ComponentPreview|PropsTable|Callout)\b/u);
});

test("the copy button puts the page's prompt on the clipboard", async ({
  context,
  page,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto(PAGE);

  await page
    .getByRole("button", { exact: true, name: "Copy prompt for AI" })
    .click();
  await expect(
    page.getByText("Prompt for Bar Visualizer copied")
  ).toBeVisible();

  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain("# Add Bar Visualizer from audiocn to this project");
  expect(copied).toContain(INSTALL);
});

test("the menu copies the install command and links out to the chat tools", async ({
  context,
  page,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto(PAGE);
  await page
    .getByRole("button", { name: "More actions for AI agents" })
    .click();

  const menu = page.getByRole("menu");
  await expect(
    menu.getByRole("menuitem", { name: "Open in v0" })
  ).toHaveAttribute("href", /v0\.dev\/chat\/api\/open\?url=.+bar-visualizer/u);

  await menu.getByRole("menuitem", { name: "Copy install command" }).click();
  // pnpm is the default the docs show.
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    "pnpm dlx shadcn@latest add @audiocn/bar-visualizer"
  );
});

test("pages that document no item have no prompt button", async ({ page }) => {
  await page.goto("/docs/concepts/decibels");
  await expect(
    page.getByRole("button", { name: "Copy prompt for AI" })
  ).toHaveCount(0);
  await page.goto("/docs/hooks/use-level");
  await expect(
    page.getByRole("button", { exact: true, name: "Copy prompt for AI" })
  ).toBeVisible();
});
