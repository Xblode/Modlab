import { Ico } from "../../../Ico";
import { useRef, useLayoutEffect, useState, useEffect, type RefObject } from "react";

export type MenuVariant = "fichier" | "loadout" | "outils" | "affichage";

export type MenuItem = {
  id: string;
  label: string;
  sub?: string;
  icon?: string;
  disabled?: boolean;
};

export type MenuPickerProps = {
  anchorRef: RefObject<HTMLElement | null>;
  variant?: MenuVariant;
  onClose: () => void;
  onVariantChange?: (variant: MenuVariant) => void;
  onItemSelect?: (item: MenuItem) => void;
  open?: boolean;
};

const VARIANTS: { variant: MenuVariant; label: string; icon: string }[] = [
  { variant: "fichier", label: "Fichier", icon: "file" },
  { variant: "loadout", label: "Loadout", icon: "layers" },
  { variant: "outils", label: "Outils", icon: "tool" },
  { variant: "affichage", label: "Affichage", icon: "settings" },
];

const ITEMS: Record<MenuVariant, MenuItem[]> = {
  fichier: [
    { id: "nouveau", label: "Nouveau", icon: "plus", sub: "Créer un nouvel élément" },
    { id: "ouvrir", label: "Ouvrir…", icon: "folder", sub: "Ouvrir un fichier existant" },
    { id: "sauvegarder", label: "Enregistrer", icon: "check", sub: "Enregistrer les modifications" },
    { id: "export", label: "Exporter…", icon: "send", sub: "Exporter les données" },
    { id: "importer", label: "Importer…", icon: "download", sub: "Importer depuis un fichier" },
    { id: "imprimer", label: "Imprimer…", icon: "file", sub: "Imprimer le contenu" },
    { id: "fermer", label: "Fermer", icon: "x", sub: "Fermer l'application" },
  ],
  loadout: [
    { id: "nouveau-loadout", label: "Nouveau loadout", icon: "plus", sub: "Créer une nouvelle instance" },
    { id: "importer-loadout", label: "Importer un loadout…", icon: "download", sub: "Depuis MO2 ou un autre format" },
    { id: "exporter-loadout", label: "Exporter un loadout…", icon: "upload", sub: "Exporter vers un fichier" },
    { id: "dupliquer-loadout", label: "Dupliquer", icon: "copy", sub: "Créer une copie de l'instance" },
    { id: "supprimer-loadout", label: "Supprimer", icon: "trash", sub: "Supprimer l'instance", disabled: true },
    { id: "renommer-loadout", label: "Renommer", icon: "edit", sub: "Modifier le nom de l'instance" },
  ],
  outils: [
    { id: "tri-loot", label: "Trier avec LOOT", icon: "wand", sub: "Réordonner les plugins" },
    { id: "xedit", label: "Ouvrir xEdit", icon: "tool", sub: "Lancer l'éditeur de plugins" },
    { id: "lancer-jeu", label: "Lancer le jeu", icon: "play", sub: "Démarrer la session" },
    { id: "terminal", label: "Terminal", icon: "term", sub: "Ouvrir un terminal" },
    { id: "console", label: "Console", icon: "prompt", sub: "Afficher la console" },
    { id: "logs", label: "Voir les logs", icon: "history", sub: "Afficher les journaux" },
    { id: "diagnostic", label: "Diagnostiquer", icon: "alert", sub: "Lancer un diagnostic" },
  ],
  affichage: [
    { id: "theme", label: "Thème", icon: "palette", sub: "Changer le thème de l'application" },
    { id: "taille-police", label: "Taille de police", icon: "text", sub: "Ajuster la taille du texte" },
    { id: "langue", label: "Langue", icon: "globe", sub: "Changer la langue de l'interface" },
    { id: "barre-outils", label: "Barre d'outils", icon: "tool", sub: "Montrer/masquer la barre d'outils" },
    { id: "barre-onglets", label: "Barre d'onglets", icon: "tab", sub: "Configuration des onglets" },
    { id: "panneau-droit", label: "Panneau droit", icon: "layers", sub: "Afficher/masquer le panneau droit" },
    { id: "panneau-bas", label: "Panneau du bas", icon: "down", sub: "Afficher/masquer le panneau du bas" },
    { id: "reset", label: "Réinitialiser", icon: "refresh", sub: "Réinitialiser les préférences" },
  ],
};
function VariantTabs({
  activeVariant,
  onVariantChange,
}: {
  activeVariant: MenuVariant;
  onVariantChange: (variant: MenuVariant) => void;
}) {
  return (
    <div className="menu-variant-tabs">
      {VARIANTS.map(({ variant, label, icon }) => (
        <button
          key={variant}
          type="button"
          className={`menu-variant-tab ${activeVariant === variant ? "active" : ""}`}
          onClick={() => onVariantChange(variant)}
          title={label}
        >
          <Ico name={icon} className="sm" />
          <span className="lbl">{label}</span>
        </button>
      ))}
    </div>
  );
}

function MenuItemRow({ item, onSelect }: { item: MenuItem; onSelect: () => void }) {
  const disabled = item.disabled ?? false;
  return (
    <button
      className={`menu-item ${disabled ? "disabled" : ""}`}
      type="button"
      onClick={disabled ? undefined : onSelect}
      disabled={disabled}
      title={item.sub ?? item.label}
    >
      {item.icon ? <Ico name={item.icon} className="sm" /> : null}
      <span className="menu-item-label">{item.label}</span>
      <span className="menu-item-sub">{item.sub}</span>
      {item.id === "fermer" ? <span className="menu-item-shortcut">⌃Q</span> : null}
    </button>
  );
}

/**
 * Generic toolbar menu: one popover, several *variants* (Fichier, Loadout,
 * Outils, Affichage) switched by the tabs at its top. `variant` is controlled
 * by the caller so the selection survives a close/reopen, and each variant's
 * items live in `ITEMS` — adding a menu means adding an entry there, not
 * another component.
 */
export function MenuPicker({
  anchorRef,
  variant = "fichier",
  onClose,
  onVariantChange,
  onItemSelect,
  open = true,
}: MenuPickerProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor || !open) return;
    const r = anchor.getBoundingClientRect();
    setPos({ x: Math.max(6, Math.min(r.left, window.innerWidth - 330)), y: r.bottom + 6 });
  }, [anchorRef, open]);

  useEffect(() => {
    function onDown(e: MouseEvent) {
      const t = e.target as Node;
      if (ref.current?.contains(t)) return;
      if (anchorRef.current?.contains(t)) return;
      onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [anchorRef, onClose]);

  if (!open || !pos) return null;
  const items = ITEMS[variant] ?? [];

  return (
    <div
      className="menu-picker"
      role="menu"
      ref={ref}
      style={{ left: pos.x, top: pos.y }}
    >
      <VariantTabs activeVariant={variant} onVariantChange={(v) => onVariantChange?.(v)} />
      <div className="menu-picker-items">
        {items.map((item) => (
          <MenuItemRow
            key={item.id}
            item={item}
            onSelect={() => {
              onItemSelect?.(item);
              onClose();
            }}
          />
        ))}
      </div>
    </div>
  );
}
