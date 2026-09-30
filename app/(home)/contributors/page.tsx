import { ArrowSquareOutIcon } from "@phosphor-icons/react/dist/ssr";
import type { Metadata } from "next";
import Image from "next/image";

import { BrandLogo } from "@/components/brand-logo";
import { Badge } from "@/components/ui/badge";
import { buttonVariants } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  fetchGitHubContributors,
  getContributorStats,
  resolveContributorsRepository,
} from "@/lib/contributors";
import type { Contributor } from "@/lib/contributors";
import { getSocialMetadata } from "@/lib/social-metadata";

const AVATAR_SIZE_PX = 48;
const DESCRIPTION =
  "Meet the people who build audiocn, the audio components for React built the shadcn way.";

// Contributors come from the GitHub API, so the page refreshes hourly on its
// own instead of waiting for a redeploy. Segment config must be a literal.
export const revalidate = 3600;

export const metadata: Metadata = {
  ...getSocialMetadata("audiocn contributors", DESCRIPTION),
  alternates: { canonical: "/contributors" },
  description: DESCRIPTION,
  title: "Contributors",
};

const numberFormatter = new Intl.NumberFormat("en-US");

const Stat = ({ label, value }: { label: string; value: number }) => (
  <div className="rounded-xl border p-6 text-center">
    <dt className="text-muted-foreground font-mono text-xs font-semibold uppercase">
      {label}
    </dt>
    <dd className="font-heading mt-2 text-4xl font-medium tabular-nums">
      {numberFormatter.format(value)}
    </dd>
  </div>
);

const SectionHeading = ({ aside, title }: { aside: string; title: string }) => (
  <div className="flex flex-col gap-3">
    <div className="flex items-baseline justify-between gap-3">
      <h2 className="text-muted-foreground font-mono text-xs font-semibold uppercase">
        {title}
      </h2>
      <p className="text-foreground truncate font-mono text-xs font-semibold">
        {aside}
      </p>
    </div>
    <Separator />
  </div>
);

const ContributorCard = ({
  contributor,
  rank,
}: {
  contributor: Contributor;
  rank: number;
}) => {
  const { avatarUrl, contributions, login, profileUrl } = contributor;
  const contributionLabel = contributions === 1 ? "commit" : "commits";

  return (
    <a
      className="group hover:border-foreground/40 hover:bg-muted/40 flex h-full w-full items-center gap-4 rounded-xl border p-4 transition-colors"
      href={profileUrl}
      rel="noopener noreferrer"
      target="_blank"
    >
      <span className="text-muted-foreground font-mono text-xs tabular-nums">
        {String(rank).padStart(2, "0")}
      </span>
      <Image
        alt=""
        className="shrink-0 rounded-full border"
        height={AVATAR_SIZE_PX}
        src={avatarUrl}
        // GitHub already serves correctly sized avatars from its own CDN.
        unoptimized
        width={AVATAR_SIZE_PX}
      />
      <span className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="font-heading truncate font-medium group-hover:underline group-hover:underline-offset-4">
          {login}
        </span>
        <span>
          <Badge variant="secondary">
            {numberFormatter.format(contributions)} {contributionLabel}
          </Badge>
        </span>
      </span>
      <ArrowSquareOutIcon
        aria-hidden="true"
        className="text-muted-foreground group-hover:text-foreground shrink-0 transition-colors"
      />
      <span className="sr-only">
        {`${login} on GitHub, ${numberFormatter.format(contributions)} ${contributionLabel}`}
      </span>
    </a>
  );
};

const ContributorsPage = async () => {
  const repository = resolveContributorsRepository();
  const contributors = await fetchGitHubContributors(repository);
  const { totalContributions, totalContributors } =
    getContributorStats(contributors);
  const repositoryUrl = `https://github.com/${repository}`;

  return (
    <div className="mx-auto w-full max-w-4xl px-4 pt-12 pb-24 sm:px-6 sm:pt-16">
      <header className="flex flex-col items-center gap-4 pb-12 text-center">
        <BrandLogo className="size-16" size={64} />
        <h1 className="font-heading text-4xl font-medium">Contributors</h1>
        <p className="text-muted-foreground max-w-xl text-balance">
          audiocn is built in the open. Every meter, fader and block exists
          because someone sent a pull request. These are the people behind them.
        </p>
      </header>

      <div className="flex flex-col gap-12">
        {contributors.length > 0 ? (
          <section aria-label="Contribution totals">
            <dl className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <Stat label="Contributors" value={totalContributors} />
              <Stat label="Commits" value={totalContributions} />
            </dl>
          </section>
        ) : null}

        <section aria-label="Contributor list">
          <SectionHeading aside={repository} title="Everyone who shipped" />
          {contributors.length > 0 ? (
            <ul className="grid gap-3 pt-4 sm:grid-cols-2">
              {contributors.map((contributor, index) => (
                <li className="flex min-w-0" key={contributor.login}>
                  <ContributorCard contributor={contributor} rank={index + 1} />
                </li>
              ))}
            </ul>
          ) : (
            <div className="mt-4 rounded-xl border border-dashed p-8 text-center">
              <p className="text-muted-foreground">
                The contributor list is unavailable right now. GitHub caches it
                hourly, so check back soon or open the repository directly.
              </p>
            </div>
          )}
        </section>

        <section aria-label="How to contribute">
          <SectionHeading aside="Open source" title="Join them" />
          <div className="mt-4 rounded-xl border p-8 text-center">
            <p className="text-muted-foreground mx-auto max-w-md">
              Every component ships with tests and a docs page, so a first
              contribution can be as small as one example or one fix. Pick an
              issue, or open one describing the component you wish audiocn had.
            </p>
            <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
              <a
                className={buttonVariants()}
                href={repositoryUrl}
                rel="noopener noreferrer"
                target="_blank"
              >
                View the repository
              </a>
              <a
                className={buttonVariants({ variant: "outline" })}
                href={`${repositoryUrl}/issues`}
                rel="noopener noreferrer"
                target="_blank"
              >
                Browse open issues
              </a>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
};

export default ContributorsPage;
