import type {
  DependencyKind,
  DependencyState,
  ResolvedDependency,
} from "../../../api";
import { Ico } from "../../../Ico";
import { Badge, type BadgeTone } from "../../ui";

/** French word for a resolved state. The badge and the Info line share it. */
export function dependencyStateLabel(state: DependencyState): string {
  switch (state) {
    case "active":
      return "active";
    case "disabled":
      return "inactive";
    case "missing":
      return "manquante";
  }
}

/** French role for a dependency kind on the Infos table. */
export function dependencyKindLabel(kind: DependencyKind | undefined): string {
  switch (kind ?? "required") {
    case "required":
      return "Requise";
    case "optional":
      return "Optionnelle";
    case "incompatible":
      return "Incompatible";
    case "discouraged":
      return "Déconseillée";
    case "usedBy":
      return "Utilisé par";
  }
}

/**
 * Declared names, with a state when the adapter resolved that name.
 * Names the adapter did not resolve stay plain — no invented state.
 * Names the adapter found and the file did not declare (a Skyrim master,
 * SKSE) are appended.
 */
export function formatDependencyLine(
  declared: string[],
  resolved: ResolvedDependency[] | undefined,
): string {
  const plain = declared.map((value) => value.trim()).filter((value) => value.length > 0);
  if (!resolved || resolved.length === 0) {
    return plain.length > 0 ? plain.join(", ") : "—";
  }
  const byName = new Map(resolved.map((dep) => [dep.name.trim().toLowerCase(), dep]));
  const used = new Set<string>();
  const parts: string[] = [];
  for (const name of plain) {
    const hit = byName.get(name.toLowerCase());
    if (hit) {
      parts.push(`${hit.name} (${dependencyStateLabel(hit.state)})`);
      used.add(hit.name.trim().toLowerCase());
    } else {
      parts.push(name);
    }
  }
  for (const dep of resolved) {
    if (!used.has(dep.name.trim().toLowerCase())) {
      parts.push(`${dep.name} (${dependencyStateLabel(dep.state)})`);
    }
  }
  return parts.length > 0 ? parts.join(", ") : "—";
}

function tooltipLine(dep: ResolvedDependency): string {
  const via =
    dep.providerName && dep.providerName !== dep.name ? ` (${dep.providerName})` : "";
  return `${dep.name} — ${dependencyStateLabel(dep.state)}${via}`;
}

/**
 * One badge for the dependency problems of an enabled mod. The caller
 * passes only `disabled` and `missing` required entries. The cell shows
 * the icon only; the tooltip lists each name and its state.
 */
export function DependencyBadge({
  deps,
  modId,
  onOpen,
}: {
  deps: ResolvedDependency[];
  modId: number;
  onOpen: (modId: number) => void;
}) {
  if (deps.length === 0) return null;
  const tone: BadgeTone = deps.some((dep) => dep.state === "missing") ? "bad" : "upd";
  const tip = deps.map(tooltipLine).join("\n");
  const target =
    deps.find((dep) => dep.state === "disabled" && dep.providerModId != null)?.providerModId ??
    modId;
  const open = () => onOpen(target);
  return (
    <Badge
      tone={tone}
      data-tip={tip}
      aria-label={tip}
      role="button"
      tabIndex={0}
      style={{ cursor: "pointer" }}
      onClick={(event) => {
        event.stopPropagation();
        open();
      }}
      onKeyDown={(event) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        event.stopPropagation();
        open();
      }}
    >
      <Ico name="alert" />
    </Badge>
  );
}
