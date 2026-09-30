import { siteConfig } from "@/lib/site";

const GITHUB_API_ORIGIN = "https://api.github.com";
const GITHUB_AVATAR_HOST = "avatars.githubusercontent.com";
const GITHUB_AVATAR_PATH_PREFIX = "/u/";
// Twice the rendered avatar size, so retina screens stay sharp without pulling
// full-resolution avatars for every contributor.
const AVATAR_REQUEST_SIZE_PX = 96;
const GITHUB_PROFILE_HOST = "github.com";
const CONTRIBUTORS_PER_PAGE = 100;
const REVALIDATE_SECONDS = 3600;
const REQUEST_TIMEOUT_MS = 3000;
// GitHub reports automation accounts alongside people; this page is about people.
const BOT_ACCOUNT_TYPE = "Bot";
const REPOSITORY_NAME = /^[\w.-]+\/[\w.-]+$/u;

export interface Contributor {
  avatarUrl: string;
  contributions: number;
  login: string;
  profileUrl: string;
}

export interface ContributorStats {
  totalContributions: number;
  totalContributors: number;
}

export interface ContributorsEnvironment {
  /** Raises the rate limit, and reads the list while the repository is private. */
  GITHUB_TOKEN?: string;
  /** Lists another public repository, in `owner/repo` form. */
  AUDIOCN_GITHUB_REPOSITORY?: string;
  readonly [key: string]: string | undefined;
}

export type FetchImplementation = (
  input: string,
  init?: RequestInit
) => Promise<Response>;

const isHttpsUrl = (
  value: string,
  host: string,
  pathPrefix: string
): boolean => {
  try {
    const url = new URL(value);
    return (
      url.protocol === "https:" &&
      url.host === host &&
      url.pathname.startsWith(pathPrefix)
    );
  } catch {
    return false;
  }
};

const toAvatarUrl = (value: string): string => {
  const avatarUrl = new URL(value);
  avatarUrl.searchParams.set("s", String(AVATAR_REQUEST_SIZE_PX));
  return avatarUrl.href;
};

/**
 * Keeps unexpected GitHub payloads from reaching the page as broken cards, and
 * keeps avatars and profile links pointed at GitHub.
 */
const toContributor = (entry: unknown): Contributor | null => {
  if (typeof entry !== "object" || entry === null) {
    return null;
  }

  const {
    avatar_url: avatarUrl,
    contributions,
    html_url: profileUrl,
    login,
    type,
  } = entry as Record<string, unknown>;

  if (
    typeof login !== "string" ||
    typeof avatarUrl !== "string" ||
    typeof profileUrl !== "string" ||
    typeof contributions !== "number"
  ) {
    return null;
  }

  const isPerson =
    login.length > 0 &&
    Number.isSafeInteger(contributions) &&
    contributions > 0 &&
    type !== BOT_ACCOUNT_TYPE;
  const pointsAtGitHub =
    isHttpsUrl(avatarUrl, GITHUB_AVATAR_HOST, GITHUB_AVATAR_PATH_PREFIX) &&
    isHttpsUrl(profileUrl, GITHUB_PROFILE_HOST, `/${login}`);

  if (!(isPerson && pointsAtGitHub)) {
    return null;
  }

  return {
    avatarUrl: toAvatarUrl(avatarUrl),
    contributions,
    login,
    profileUrl,
  };
};

const byContributionsThenLogin = (
  first: Contributor,
  second: Contributor
): number =>
  second.contributions - first.contributions ||
  first.login.localeCompare(second.login);

/** The configured repository when it is a valid `owner/repo`, else audiocn's. */
export const resolveContributorsRepository = (
  environment: ContributorsEnvironment = process.env
): string => {
  const configured = environment.AUDIOCN_GITHUB_REPOSITORY?.trim();
  return configured && REPOSITORY_NAME.test(configured)
    ? configured
    : siteConfig.githubRepo;
};

/**
 * Contributors for a repository, ranked by commits. Resolves to an empty list
 * whenever GitHub is unreachable or the repository is still private, so the
 * page always renders. Responses are cached for an hour.
 */
export const fetchGitHubContributors = async (
  repository: string = resolveContributorsRepository(),
  fetchImplementation: FetchImplementation = fetch,
  environment: ContributorsEnvironment = process.env
): Promise<Contributor[]> => {
  try {
    const headers = new Headers({
      Accept: "application/vnd.github+json",
      "User-Agent": siteConfig.name,
      "X-GitHub-Api-Version": "2022-11-28",
    });
    const token = environment.GITHUB_TOKEN;
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }

    const response = await fetchImplementation(
      `${GITHUB_API_ORIGIN}/repos/${repository}/contributors?per_page=${CONTRIBUTORS_PER_PAGE}`,
      {
        headers,
        next: { revalidate: REVALIDATE_SECONDS },
        redirect: "error",
        signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
      }
    );

    if (!response.ok) {
      return [];
    }

    const payload: unknown = await response.json();
    if (!Array.isArray(payload)) {
      return [];
    }

    const contributors: Contributor[] = [];
    for (const entry of payload) {
      const contributor = toContributor(entry);
      if (contributor !== null) {
        contributors.push(contributor);
      }
    }

    return contributors.toSorted(byContributionsThenLogin);
  } catch {
    return [];
  }
};

export const getContributorStats = (
  contributors: readonly Contributor[]
): ContributorStats => {
  let totalContributions = 0;
  for (const contributor of contributors) {
    totalContributions += contributor.contributions;
  }
  return { totalContributions, totalContributors: contributors.length };
};
