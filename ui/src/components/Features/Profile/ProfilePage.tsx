import { Ico } from "../../../Ico";
import { Badge, Crumbs, Panel, Switch } from "../../ui";

/**
 * Deterministic pseudo-random stream (mulberry32) — the profile heatmap
 * needs numbers that look random but are stable across renders (and across
 * reloads, since there's no Core data behind them yet); `Math.random()`
 * would repaint the mock grid differently every render.
 */
function mulberry32(seed: number): () => number {
  let a = seed;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** ~52 weeks × 7 days of mock activity intensity (0-4), seeded so the same
 * grid renders every time — a stand-in for a future Core-backed "days with
 * at least one mutation" series. */
function mockActivityWeeks(): number[][] {
  const rand = mulberry32(20260921);
  const weeks: number[][] = [];
  for (let w = 0; w < 52; w += 1) {
    const days: number[] = [];
    for (let d = 0; d < 7; d += 1) {
      const r = rand();
      days.push(r > 0.72 ? 3 + Math.round(rand()) : r > 0.45 ? 1 + Math.round(rand()) : 0);
    }
    weeks.push(days);
  }
  return weeks;
}

const PROFILE_STATS: readonly (readonly [string, string, string])[] = [
  ["Mods installés", "187", "box"],
  ["Heures de jeu", "412 h", "play"],
  ["Crashs cumulés", "23", "alert"],
  ["Packs publiés", "3", "share"],
];

const PROFILE_TROPHIES: readonly {
  label: string;
  desc: string;
  secret?: boolean;
}[] = [
  { label: "Load order propre", desc: "0 conflit non résolu pendant 30 jours" },
  { label: "Sauveur de save", desc: "Rollback déclenché juste avant un crash" },
  { label: "Architecte de pack", desc: "Un ModPack forké par 10 autres joueurs" },
  { label: "???", desc: "Titre secret — pas encore débloqué", secret: true },
  { label: "???", desc: "Titre secret — pas encore débloqué", secret: true },
];

const PROFILE_LINEAGE: readonly (readonly [string, string, "root" | "upd" | "plug"])[] = [
  ["Wabbajack Skyrim SE", "v2.1", "root"],
  ["Fork de Benjamin", "v2.1.4", "upd"],
  ["Branche « QA fixtures »", "v2.1.4-qa3", "plug"],
];

/** One `.heatmap` activity grid — 52 columns of 7 days, coloured by mock
 * intensity. Purely decorative (`aria-hidden`): nothing here is announced,
 * same reasoning as `Avatar`'s image fallback. */
function ActivityHeatmap() {
  const weeks = mockActivityWeeks();
  return (
    <div className="heatmap" aria-hidden="true">
      {weeks.map((days, wi) => (
        <div className="heatmap-col" key={wi}>
          {days.map((level, di) => (
            <span key={di} className={`heatmap-cell level-${level}`} />
          ))}
        </div>
      ))}
    </div>
  );
}

/**
 * The user's own profile — a dedicated page reached from the sidebar's
 * avatar, not a Réglages subsection: this is identity, not configuration,
 * and (eventually) something every account has one of, not a preference you
 * tune. The rich part (stats, activity, titles, lineage) still sits behind
 * the "Communauté" flag from `docs/deep-research-report.md` (toggled in
 * Réglages → Général) — only the account row is unconditional.
 */
export function ProfilePage({
  communityUpdate,
  onToggleCommunityUpdate,
}: {
  communityUpdate: boolean;
  onToggleCommunityUpdate: () => void;
}) {
  const accountRow = (
    <div className="settings-group">
      <div className="settings-row">
        <div className="settings-row-text">
          <div className="settings-row-title">Compte</div>
          <div className="settings-row-desc">
            ModLab est local et personnel : pas de compte, pas de synchronisation
            cloud. Ta modlist ne quitte cette machine que lorsque tu déclenches
            toi-même un appel à un Provider (Nexus, CurseForge).
          </div>
        </div>
        <div className="settings-row-control">
          <Badge tone="root">local</Badge>
        </div>
      </div>
    </div>
  );

  return (
    <>
      <Crumbs path={["Profil"]} />
      <div className="settings-page">
        {!communityUpdate ? (
          <>
            <Panel icon="user" title="Profil communautaire" bodyClassName="why">
              <div>
                Active la mise à jour communautaire pour voir ton profil complet :
                stats, historique d'activité, titres et lignée de tes ModPacks.
              </div>
              <div className="settings-row" style={{ padding: "10px 0 0" }}>
                <div className="settings-row-text">
                  <div className="settings-row-title">Mises à jour communautaires</div>
                </div>
                <div className="settings-row-control">
                  <Switch checked={communityUpdate} onToggle={onToggleCommunityUpdate} />
                </div>
              </div>
            </Panel>
            {accountRow}
          </>
        ) : (
          <>
            <div className="settings-group">
              <div className="prof-head">
                <span className="prof-avatar" aria-hidden="true">BP</span>
                <div className="profile-head-text">
                  <div className="prof-name">Benjamin</div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    Modder Skyrim depuis 2023 · aucune donnée envoyée hors de cette machine
                  </div>
                </div>
              </div>
              <div className="prof-stats">
                {PROFILE_STATS.map(([label, value, icon]) => (
                  <div className="stat" key={label}>
                    <span className="k">{label}</span>
                    <span className="v">
                      <Ico name={icon} />
                      {value}
                    </span>
                  </div>
                ))}
              </div>
            </div>
            <div className="settings-group">
              <div className="settings-group-title">Activité</div>
              <ActivityHeatmap />
            </div>
            <div className="settings-group">
              <div className="settings-group-title">Titres</div>
              <div className="chip-row">
                {PROFILE_TROPHIES.map((t) => (
                  <span
                    key={t.label}
                    className={`trophy-chip${t.secret ? " secret" : ""}`}
                    title={t.desc}
                  >
                    <Ico name={t.secret ? "shield" : "sparkle"} className="sm" />
                    {t.label}
                  </span>
                ))}
              </div>
            </div>
            <div className="settings-group">
              <div className="settings-group-title">Lignée</div>
              <div className="chip-row">
                {PROFILE_LINEAGE.map(([name, version, tone]) => (
                  <Badge tone={tone} key={name} className="lineage-chip">
                    {name} · {version}
                  </Badge>
                ))}
              </div>
            </div>
            {accountRow}
          </>
        )}
      </div>
    </>
  );
}
