import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-shell";

const REPO_OWNER = "lennoxrose";
const REPO_NAME = "HyprWall";
const REPO_URL = `https://github.com/${REPO_OWNER}/${REPO_NAME}`;

const CACHE_KEY = "hyprwall-credits-cache";
const CACHE_TTL_MS = 10 * 60 * 1000;

interface GitHubUser {
  login: string;
  avatar_url: string;
  html_url: string;
  contributions?: number;
}

interface CreditsCache {
  fetchedAt: number;
  owner: GitHubUser;
  contributors: GitHubUser[];
}

/** `localStorage` survives across settings-modal opens and app restarts,
 * which is the point -- a 10-minute cache is meant to outlive a single
 * component mount, not just dedupe renders. Best-effort: any read/parse/
 * write failure (private browsing quirks, corrupt JSON, quota) just means
 * "no cache," never a crash. */
function readCache(): CreditsCache | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as CreditsCache;
    if (Date.now() - parsed.fetchedAt > CACHE_TTL_MS) return null;
    return parsed;
  } catch {
    return null;
  }
}

function writeCache(owner: GitHubUser, contributors: GitHubUser[]) {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify({ fetchedAt: Date.now(), owner, contributors }));
  } catch {
    // Caching is best-effort -- a write failure just means next open re-fetches.
  }
}

const AVATAR_STYLE = {
  borderRadius: "50%",
  border: "1px solid #333",
};

function ContributeLink() {
  return (
    <button
      onClick={() => open(REPO_URL).catch(() => {})}
      style={{
        background: "transparent",
        border: "1px solid #4ade80",
        borderRadius: 6,
        padding: "6px 14px",
        fontSize: 12,
        fontWeight: 600,
        color: "#4ade80",
        cursor: "pointer",
      }}
    >
      I want to Contribute
    </button>
  );
}

/** Credits tab content: the repo owner (fetched from the repo's own `owner`
 * field rather than hardcoded, so it stays correct if that ever changes)
 * plus everyone else GitHub's contributors API knows about (the owner is
 * excluded from that list -- they're already the header), sorted by commit
 * count. Entirely self-contained -- this is the one place in the app that
 * makes a network call (public, unauthenticated GitHub API), and a failure
 * here only empties this one tab, never anything else. Results are cached
 * for 10 minutes (see `readCache`/`writeCache`) so opening this tab
 * repeatedly doesn't hammer GitHub's API. */
export function CreditsPanel() {
  const [owner, setOwner] = useState<GitHubUser | null>(null);
  const [contributors, setContributors] = useState<GitHubUser[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const cached = readCache();
    if (cached) {
      setOwner(cached.owner);
      setContributors(cached.contributors);
      setLoading(false);
      return;
    }

    let cancelled = false;

    const fetchJson = (path: string) =>
      fetch(`https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}${path}`).then((res) => {
        if (!res.ok) throw new Error(`GitHub API returned ${res.status} for ${path}`);
        return res.json();
      });

    Promise.all([fetchJson(""), fetchJson("/contributors")])
      .then(([repo, contribs]: [{ owner: GitHubUser }, GitHubUser[]]) => {
        if (cancelled) return;
        const sorted = [...contribs].sort((a, b) => (b.contributions ?? 0) - (a.contributions ?? 0));
        setOwner(repo.owner);
        setContributors(sorted);
        writeCache(repo.owner, sorted);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(String(err instanceof Error ? err.message : err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) {
    return <p style={{ fontSize: 13, color: "#888" }}>loading credits...</p>;
  }

  if (error) {
    return (
      <p style={{ fontSize: 13, color: "#f87171" }} role="alert">
        couldn't reach GitHub to load credits: {error}
      </p>
    );
  }

  const otherContributors = contributors.filter((c) => c.login !== owner?.login);
  const hasContributors = otherContributors.length > 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      {owner && (
        <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 4 }}>
          <img src={owner.avatar_url} alt={owner.login} style={{ ...AVATAR_STYLE, width: 72, height: 72 }} />
          <div style={{ fontSize: 15, fontWeight: 700, color: "#fff" }}>{owner.login}</div>
          <div style={{ fontSize: 11, color: "#666" }}>Creator of HyprWall</div>
        </div>
      )}

      <div>
        <div
          style={{
            fontSize: 12,
            fontWeight: 600,
            color: "#888",
            marginBottom: 8,
            textTransform: "uppercase",
            letterSpacing: 0.5,
            textAlign: "center",
          }}
        >
          Thanks to everyone who's contributed
        </div>

        {hasContributors ? (
          <>
            <div style={{ display: "flex", flexWrap: "wrap", justifyContent: "center", gap: 12 }}>
              {otherContributors.map((c) => (
                <div
                  key={c.login}
                  title={`${c.login} -- ${c.contributions ?? 0} commit${c.contributions === 1 ? "" : "s"}`}
                  style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 4, width: 64 }}
                >
                  <img src={c.avatar_url} alt={c.login} style={{ ...AVATAR_STYLE, width: 40, height: 40 }} />
                  <span
                    style={{
                      fontSize: 10,
                      color: "#ccc",
                      textAlign: "center",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                      width: "100%",
                    }}
                  >
                    {c.login}
                  </span>
                </div>
              ))}
            </div>
            <div style={{ display: "flex", justifyContent: "center", marginTop: 20 }}>
              <ContributeLink />
            </div>
          </>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10 }}>
            <p style={{ fontSize: 12, color: "#666", margin: 0 }}>Sadly no Contributors here yet</p>
            <ContributeLink />
          </div>
        )}
      </div>
    </div>
  );
}
