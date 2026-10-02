import { GitHubStars } from "@/components/docs/github-stars";
import { siteConfig } from "@/lib/site";

const ONE_HOUR_IN_SECONDS = 3600;

const getStargazersCount = async (): Promise<number> => {
  try {
    const response = await fetch(
      `https://api.github.com/repos/${siteConfig.githubRepo}`,
      {
        headers: { Accept: "application/vnd.github+json" },
        next: { revalidate: ONE_HOUR_IN_SECONDS },
      }
    );
    if (!response.ok) {
      return 0;
    }
    const data: { stargazers_count?: unknown } = await response.json();
    return typeof data.stargazers_count === "number"
      ? data.stargazers_count
      : 0;
  } catch {
    return 0;
  }
};

/** Navbar link to the repo with a server-fetched star count, revalidated hourly. */
export const GitHubStarsLink = async () => {
  const stargazersCount = await getStargazersCount();

  return (
    <GitHubStars
      repo={siteConfig.githubRepo}
      stargazersCount={stargazersCount}
    />
  );
};
