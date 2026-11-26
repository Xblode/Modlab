import type { PopoverMenuEntry } from "../../ui";

/**
 * Application hamburger menu tree. Top level used to be the four tabs
 * (Fichier / Loadout / Outils / Affichage); children open as flyouts.
 */
export const APP_MENU_TREE: PopoverMenuEntry[] = [
  {
    id: "fichier",
    label: "Fichier",
    icon: "file",
    children: [
      { id: "nouveau", label: "Nouveau", icon: "plus", sub: "Créer un nouvel élément" },
      { id: "ouvrir", label: "Ouvrir…", icon: "folder", sub: "Ouvrir un fichier existant" },
      { id: "sauvegarder", label: "Enregistrer", icon: "check", sub: "Enregistrer les modifications" },
      { id: "export", label: "Exporter…", icon: "send", sub: "Exporter les données" },
      { id: "importer", label: "Importer…", icon: "download", sub: "Importer depuis un fichier" },
      { id: "imprimer", label: "Imprimer…", icon: "file", sub: "Imprimer le contenu" },
      { id: "fermer", label: "Fermer", icon: "x", sub: "Fermer l'application", shortcut: "⌃Q" },
    ],
  },
  {
    id: "loadout",
    label: "Loadout",
    icon: "layers",
    children: [
      {
        id: "nouveau-loadout",
        label: "Nouveau loadout",
        icon: "plus",
        sub: "Créer une nouvelle instance",
      },
      {
        id: "importer-loadout",
        label: "Importer un loadout…",
        icon: "download",
        sub: "Depuis MO2 ou un autre format",
      },
      {
        id: "exporter-loadout",
        label: "Exporter un loadout…",
        icon: "upload",
        sub: "Exporter vers un fichier",
      },
      {
        id: "dupliquer-loadout",
        label: "Dupliquer",
        icon: "copy",
        sub: "Créer une copie de l'instance",
      },
      {
        id: "supprimer-loadout",
        label: "Supprimer",
        icon: "trash",
        sub: "Supprimer l'instance",
        disabled: true,
      },
      {
        id: "renommer-loadout",
        label: "Renommer",
        icon: "edit",
        sub: "Modifier le nom de l'instance",
      },
    ],
  },
  {
    id: "outils",
    label: "Outils",
    icon: "tool",
    children: [
      { id: "tri-loot", label: "Trier avec LOOT", icon: "wand", sub: "Réordonner les plugins" },
      { id: "xedit", label: "Ouvrir xEdit", icon: "tool", sub: "Lancer l'éditeur de plugins" },
      { id: "lancer-jeu", label: "Lancer le jeu", icon: "play", sub: "Démarrer la session" },
      { id: "terminal", label: "Terminal", icon: "term", sub: "Ouvrir un terminal" },
      { id: "console", label: "Console", icon: "prompt", sub: "Afficher la console" },
      { id: "logs", label: "Voir les logs", icon: "history", sub: "Afficher les journaux" },
      { id: "diagnostic", label: "Diagnostiquer", icon: "alert", sub: "Lancer un diagnostic" },
    ],
  },
  {
    id: "affichage",
    label: "Affichage",
    icon: "settings",
    children: [
      {
        id: "theme",
        label: "Thème",
        icon: "palette",
        sub: "Changer le thème de l'application",
        children: [
          { id: "theme-clair", label: "Clair", icon: "sparkle" },
          { id: "theme-sombre", label: "Sombre", icon: "disc" },
          { id: "theme-auto", label: "Auto", icon: "refresh" },
        ],
      },
      {
        id: "taille-police",
        label: "Taille de police",
        icon: "text",
        sub: "Ajuster la taille du texte",
      },
      { id: "langue", label: "Langue", icon: "globe", sub: "Changer la langue de l'interface" },
      {
        id: "barre-outils",
        label: "Barre d'outils",
        icon: "tool",
        sub: "Montrer/masquer la barre d'outils",
      },
      {
        id: "barre-onglets",
        label: "Barre d'onglets",
        icon: "tab",
        sub: "Configuration des onglets",
      },
      {
        id: "panneau-droit",
        label: "Panneau droit",
        icon: "layers",
        sub: "Afficher/masquer le panneau droit",
      },
      {
        id: "panneau-bas",
        label: "Panneau du bas",
        icon: "down",
        sub: "Afficher/masquer le panneau du bas",
      },
      {
        id: "reset",
        label: "Réinitialiser",
        icon: "refresh",
        sub: "Réinitialiser les préférences",
      },
    ],
  },
];
