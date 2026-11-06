import { Ico } from "../../../Ico";
import { Badge, Crumbs, Panel } from "../../ui";

const MARKET_ITEMS: readonly {
  name: string;
  kind: "frame" | "theme";
  kindLabel: string;
  icon: string;
  price: number;
  owned: boolean;
}[] = [
  { name: "Cadre Dragonborn", kind: "frame", kindLabel: "cadre d'avatar", icon: "shield", price: 120, owned: false },
  { name: "Cadre Argent", kind: "frame", kindLabel: "cadre d'avatar", icon: "star", price: 60, owned: true },
  { name: "Thème Nordique", kind: "theme", kindLabel: "thème d'interface", icon: "palette", price: 200, owned: false },
  { name: "Thème Ambre", kind: "theme", kindLabel: "thème d'interface", icon: "palette", price: 200, owned: false },
  { name: "Cadre Whiterun", kind: "frame", kindLabel: "cadre d'avatar", icon: "flag", price: 90, owned: false },
  { name: "Thème Contraste", kind: "theme", kindLabel: "thème d'interface", icon: "disc", price: 150, owned: true },
];

/** Communauté → Boutique — a static preview of the cosmetic marketplace
 * from `docs/deep-research-report.md`. Own page, own nav entry
 * (`CommunityMenu`): not a Réglages subsection. */
export function MarketplacePage({ onUnwired }: { onUnwired: (label: string) => void }) {
  return (
    <>
      <Crumbs path={["Communauté", "Boutique"]} />
      <div className="settings-page">
        <Panel icon="box" title="Aperçu, pas encore branché" bodyClassName="why">
          Prévisualisation de la boutique cosmétique décrite dans le rapport
          communauté — pas d'économie réelle : aucun ModPoint n'existe côté
          Core, tout ici est un aperçu visuel.
        </Panel>
        <div className="settings-group">
          <div className="settings-group-title">Objets cosmétiques</div>
          <div className="market-grid">
            {MARKET_ITEMS.map((item) => (
              <div className="market-card" key={item.name}>
                <div className={`market-thumb ${item.kind}`} aria-hidden="true">
                  <Ico name={item.icon} />
                </div>
                <div className="market-card-name">{item.name}</div>
                <div className="muted" style={{ fontSize: 11 }}>{item.kindLabel}</div>
                <div className="market-card-foot">
                  {item.owned ? (
                    <Badge tone="root">possédé</Badge>
                  ) : (
                    <span className="market-price">
                      <Ico name="sparkle" className="sm" />
                      {item.price} MP
                    </span>
                  )}
                  <button
                    type="button"
                    className="btn ghost"
                    title="Pas encore branché au Core"
                    onClick={() =>
                      onUnwired(item.owned ? `Équiper « ${item.name} »` : `Acheter « ${item.name} »`)
                    }
                  >
                    {item.owned ? "Équiper" : "Acheter"}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}
