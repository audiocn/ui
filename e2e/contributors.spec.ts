import { expect, test } from "@playwright/test";

const REPOSITORY_URL = "https://github.com/TheOrcDev/audiocn";

for (const url of ["/", "/docs/components/fader"]) {
  test(`the footer on ${url} credits the authors and links to the contributors`, async ({
    page,
  }) => {
    await page.goto(url);
    const footer = page.getByRole("contentinfo");
    await expect(footer).toContainText("Built by fortysevenfx and orcdev");
    await expect(
      footer.getByRole("link", { name: "fortysevenfx" })
    ).toHaveAttribute("href", "https://x.com/fortysevenfx");
    await expect(footer.getByRole("link", { name: "orcdev" })).toHaveAttribute(
      "href",
      "https://x.com/orcdev"
    );
    await expect(
      footer.getByRole("navigation", { name: "Secondary" }).getByRole("link")
    ).toHaveText(["Contributors"]);
  });
}

test("the contributors page lists people or explains why it cannot", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("contentinfo")
    .getByRole("link", { name: "Contributors" })
    .click();
  await expect(page).toHaveURL(/\/contributors$/u);
  await expect(
    page.getByRole("heading", { level: 1, name: "Contributors" })
  ).toBeVisible();

  // GitHub may be unreachable or rate limited, so either state is valid.
  const list = page.getByRole("region", { name: "Contributor list" });
  await expect(
    list
      .getByRole("link")
      .first()
      .or(list.getByText("The contributor list is unavailable right now."))
  ).toBeVisible();

  const join = page.getByRole("region", { name: "How to contribute" });
  await expect(
    join.getByRole("link", { name: "View the repository" })
  ).toHaveAttribute("href", REPOSITORY_URL);
  await expect(
    join.getByRole("link", { name: "Browse open issues" })
  ).toHaveAttribute("href", `${REPOSITORY_URL}/issues`);
});
