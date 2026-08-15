import { useEffect, useState } from "react";

const REPO_OWNER = "lennoxrose";
const REPO_NAME = "HyprWall";

interface GitHubUser {
  login: string;
  avatar_url: string;
  html_url: string;
  contributions?: number;
}

const SECTION_LABEL_STYLE = {
  fontSize: 12,
  fontWeight: 600,
  color: "#888",
  marginBottom: 8,
  textTransform: "uppercase" as const,
  letterSpacing: 0.5,
};

const AVATAR_STYLE = {
  borderRadius: "50%",
  border: "1px solid #333",
};

/** Credits tab content: the repo owner (fetched from the repo's own `owner`
 * field rather than hardcoded, so it stays correct if that ever changes)
 * plus everyone GitHub's contributors API knows about, sorted by commit
 * count. Entirely self-contained -- this is the one place in the app that
 * makes a network call (public, unauthenticated GitHub API), and a failure
 * here only empties this one tab, never anything else. */
export function CreditsPanel() {
  const [owner, setOwner] = useState<GitHubUser | null>(null);
  const [contributors, setContributors] = useState<GitHubUser[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    const fetchJson = (path: string) =>
      fetch(`https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}${path}`).then((res) => {
        if (!res.ok) throw new Error(`GitHub API returned ${res.status} for ${path}`);
        return res.json();
      });

    Promise.all([fetchJson(""), fetchJson("/contributors")])
      .then(([repo, contribs]: [{ owner: GitHubUser }, GitHubUser[]]) => {
        if (cancelled) return;
        setOwner(repo.owner);
        setContributors([...contribs].sort((a, b) => (b.contributions ?? 0) - (a.contributions ?? 0)));
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

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
      {owner && (
        <div>
          <div style={SECTION_LABEL_STYLE}>Maintainer</div>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <img src={owner.avatar_url} alt={owner.login} style={{ ...AVATAR_STYLE, width: 48, height: 48 }} />
            <div>
              <div style={{ fontSize: 14, fontWeight: 600, color: "#fff" }}>{owner.login}</div>
              <div style={{ fontSize: 11, color: "#666" }}>Creator of HyprWall</div>
            </div>
          </div>
        </div>
      )}

      <div>
        <div style={SECTION_LABEL_STYLE}>Thanks to everyone who's contributed</div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
          {contributors.map((c) => (
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
      </div>
    </div>
  );
}
