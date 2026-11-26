import { useRef, useState, type ReactNode } from "react";
import { Ico } from "../../../Ico";
import {
  Accordion,
  Badge,
  type BadgeTone,
  Chip,
  type ChipTone,
  Button,
  type ButtonVariant,
  ChannelDot,
  Checkbox,
  ConfirmDialog,
  Crumbs,
  Dialog,
  FavoriteButton,
  IconButton,
  Input,
  type InputVariant,
  Panel,
  PixelLoader,
  PopoverMenu,
  PopoverMenuEntries,
  PopoverMenuFooterLink,
  PopoverMenuItem,
  PopoverMenuSep,
  Radio,
  SearchInput,
  Select,
  SettingsRow,
  SideBack,
  SideEmpty,
  SideRow,
  Switch,
  Toast,
} from "../../ui";
import type { Channel } from "../../../api";

/** Representative icon for each Badge tone (Flags / status usage). */
const BADGE_TONE_ICO: Record<BadgeTone, string> = {
  win: "flag",
  lose: "flag",
  both: "zap",
  plug: "file",
  upd: "alert",
  bad: "alert",
  root: "info",
  archived: "box",
  downloading: "download",
};

type ColorToken = { token: string; useFr: string; hex: string };

/** Dark-first palette from `:root` in shell.css — product default, not light. */
const COLOR_FAMILIES: { title: string; colors: ColorToken[] }[] = [
  {
    title: "Surfaces",
    colors: [
      { token: "--c1", useFr: "surface 1", hex: "#1b1a19" },
      { token: "--c2", useFr: "surface 2", hex: "#232221" },
      { token: "--c3", useFr: "surface 3", hex: "#2a2928" },
      { token: "--c4", useFr: "surface 4", hex: "#323130" },
      { token: "--c5", useFr: "surface 5", hex: "#3c3a38" },
      { token: "--bg-app", useFr: "fond app / sidebar", hex: "→ --c1" },
      { token: "--bg-content", useFr: "fond contenu", hex: "→ --c2" },
      { token: "--bg-raised", useFr: "cartes / panels", hex: "→ --c3" },
      { token: "--bg-utility", useFr: "barre utilitaire", hex: "→ --c4" },
      { token: "--bg-overlay", useFr: "menus / dialogs", hex: "#2e2d2b" },
      { token: "--hover-ov", useFr: "survol", hex: "→ --c5" },
    ],
  },
  {
    title: "Texte",
    colors: [
      { token: "--t1", useFr: "texte principal", hex: "#e8e6e2" },
      { token: "--t2", useFr: "texte secondaire", hex: "#aaa59e" },
      { token: "--t3", useFr: "texte atténué", hex: "#938e86" },
      { token: "--t4", useFr: "texte discret", hex: "#4a4844" },
    ],
  },
  {
    title: "Lignes / sélection",
    colors: [
      { token: "--line", useFr: "bordure fine", hex: "rgba(255,255,255,.10)" },
      { token: "--line2", useFr: "bordure forte", hex: "rgba(255,255,255,.15)" },
      { token: "--sel", useFr: "fond sélection", hex: "rgba(90,156,245,.14)" },
      { token: "--sel-line", useFr: "contour sélection", hex: "rgba(90,156,245,.55)" },
    ],
  },
  {
    title: "Sémantique",
    colors: [
      { token: "--accent", useFr: "accent UI", hex: "#5a9cf5" },
      { token: "--on-accent", useFr: "encre sur accent", hex: "#0f1620" },
      { token: "--green", useFr: "succès", hex: "#3fb950" },
      { token: "--green-btn", useFr: "bouton primaire", hex: "#238636" },
      { token: "--green-btn-h", useFr: "bouton primaire hover", hex: "#2ea043" },
      { token: "--warn", useFr: "avertissement", hex: "#d29922" },
      { token: "--danger", useFr: "danger", hex: "#f85149" },
      { token: "--info", useFr: "info", hex: "#5a9cf5" },
      { token: "--win", useFr: "winning override", hex: "#39d0d8" },
      { token: "--lose", useFr: "losing override", hex: "#f778ba" },
      { token: "--gen", useFr: "génération IA", hex: "#a371f7" },
      { token: "--claude", useFr: "Chat / Claude", hex: "#d97757" },
      { token: "--run", useFr: "session en cours", hex: "#6e3a27" },
    ],
  },
];

/** Soft dark fills (Badge / Chip) — same bases, tinted into --c1. */
const SOFT_SWATCHES: { base: string; useFr: string }[] = [
  { base: "--accent", useFr: "Chip plan / accent soft" },
  { base: "--warn", useFr: "Chip debug / Badge upd" },
  { base: "--gen", useFr: "Chip multi / IA" },
  { base: "--win", useFr: "Badge win" },
  { base: "--lose", useFr: "Badge lose" },
  { base: "--danger", useFr: "Badge bad" },
  { base: "--info", useFr: "Badge archived" },
  { base: "--t2", useFr: "Badge neutre / ghost base" },
];

const RADIUS_TOKENS: { token: string; value: string; useFr: string }[] = [
  { token: "--r", value: "12px", useFr: "cartes, panels, dialogs, popovers" },
  { token: "--r-ctl", value: "8px", useFr: "boutons, selects, inputs" },
  { token: "--r-tag", value: "6px", useFr: "badges, pastilles" },
];

const BORDER_TOKENS: { token: string; value: string; useFr: string }[] = [
  { token: "--line", value: "1px solid", useFr: "séparateur / contour discret" },
  { token: "--line2", value: "1px solid", useFr: "contour contrôle / champ" },
  { token: "--sel-line", value: "1px solid", useFr: "focus / sélection" },
];

function DsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="ds-section">
      <h2 className="ds-section-title">{title}</h2>
      {children}
    </section>
  );
}

function DsRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="ds-row">
      <code className="ds-label">{label}</code>
      <div className="ds-preview">{children}</div>
    </div>
  );
}

function DsColorChip({ token, useFr, hex }: ColorToken) {
  return (
    <div className="ds-color-chip">
      <span
        className="ds-swatch"
        aria-hidden
        style={{ background: `var(${token})` }}
        data-tip={`${token} · ${useFr} · ${hex}`}
      />
      <code className="ds-color-token">{token}</code>
      <span className="ds-color-use">{useFr}</span>
      <code className="ds-color-hex">{hex}</code>
    </div>
  );
}

function DsSoftSwatch({ base, useFr }: { base: string; useFr: string }) {
  return (
    <div className="ds-color-chip">
      <span
        className="ds-swatch ds-swatch-soft"
        aria-hidden
        style={{
          background: `color-mix(in srgb, var(${base}) 16%, var(--c1))`,
          color: `color-mix(in srgb, var(${base}) 68%, #fff)`,
        }}
        data-tip={`soft · ${base} · ${useFr}`}
      />
      <code className="ds-color-token">{base}</code>
      <span className="ds-color-use">{useFr}</span>
      <code className="ds-color-hex">16% → --c1</code>
    </div>
  );
}

export function DesignSystemPage() {
  const [swOn, setSwOn] = useState(true);
  const [chkOn, setChkOn] = useState(true);
  const [chkModOn, setChkModOn] = useState(true);
  const [favOn, setFavOn] = useState(false);
  const [radioOn, setRadioOn] = useState(true);
  const [search, setSearch] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmDangerOpen, setConfirmDangerOpen] = useState(false);
  const [selectOpen, setSelectOpen] = useState(false);
  const [selectValue, setSelectValue] = useState("vanilla");
  const selectRef = useRef<HTMLButtonElement>(null);
  const selectOptions = [
    { id: "vanilla", label: "Vanilla", meta: "r.1" },
    { id: "perf", label: "Performance", meta: "r.4" },
    { id: "quest", label: "Quêtes", meta: "r.2" },
  ] as const;
  const selectCurrent =
    selectOptions.find((o) => o.id === selectValue) ?? selectOptions[0];

  const buttonVariants: ButtonVariant[] = [
    "default",
    "ghost",
    "sm",
    "sm-ghost",
    "action",
    "round",
    "ghost-round",
  ];
  const badgeTones: BadgeTone[] = [
    "win",
    "lose",
    "both",
    "plug",
    "upd",
    "bad",
    "root",
    "archived",
    "downloading",
  ];
  const channels: Channel[] = ["stable", "beta", "alpha"];
  const inputVariants: InputVariant[] = ["ghost", "field"];

  return (
    <div className="content">
      <div className="settings-page design-system-page">
        <h1>Design system</h1>

        <DsSection title="Couleurs">
          <p className="ds-section-note">
            Mode sombre = défaut produit (<code>:root</code> dans shell.css). Les hex ci-dessous
            sont la référence dark-first — pas un thème clair.{" "}
            <code>{`data-theme="light"`}</code> n’est qu’une surcharge optionnelle, hors scope de
            ce design system.
          </p>
          {COLOR_FAMILIES.map((family) => (
            <div key={family.title} className="ds-colors-family">
              <h3 className="ds-colors-family-title">{family.title}</h3>
              <div className="ds-colors">
                {family.colors.map((c) => (
                  <DsColorChip key={c.token} {...c} />
                ))}
              </div>
            </div>
          ))}
          <div className="ds-colors-family">
            <h3 className="ds-colors-family-title">Soft Badge / Chip</h3>
            <p className="ds-section-note">
              Recette soft dark (Badge et Chip) : fond{" "}
              <code>color-mix(base 16%, var(--c1))</code> — teinte dans le charcoal, pas un wash
              pastel transparent ; foreground{" "}
              <code>color-mix(base 68%, #fff)</code> — plus clair que le fill. Pas de bordure.
            </p>
            <div className="ds-colors">
              {SOFT_SWATCHES.map((s) => (
                <DsSoftSwatch key={s.base + s.useFr} {...s} />
              ))}
            </div>
          </div>
        </DsSection>

        <DsSection title="Mesures">
          <h3 className="ds-colors-family-title">Rayons</h3>
          <div className="ds-measures">
            {RADIUS_TOKENS.map((m) => (
              <div key={m.token} className="ds-measure-chip">
                <span
                  className="ds-measure-swatch"
                  style={{ borderRadius: `var(${m.token})` }}
                  aria-hidden
                />
                <span className="ds-measure-meta">
                  <code className="ds-measure-token">
                    {m.token} · {m.value}
                  </code>
                  <span className="ds-measure-use">{m.useFr}</span>
                </span>
              </div>
            ))}
          </div>
          <h3 className="ds-colors-family-title" style={{ marginTop: 14 }}>
            Bordures
          </h3>
          <div className="ds-measures">
            {BORDER_TOKENS.map((m) => (
              <div key={m.token} className="ds-measure-chip">
                <span
                  className="ds-measure-swatch"
                  style={{
                    borderRadius: "var(--r-ctl)",
                    border: `1px solid var(${m.token})`,
                    background: "transparent",
                  }}
                  aria-hidden
                />
                <span className="ds-measure-meta">
                  <code className="ds-measure-token">{m.token}</code>
                  <span className="ds-measure-use">
                    {m.useFr} · {m.value}
                  </span>
                </span>
              </div>
            ))}
          </div>
        </DsSection>

        <DsSection title="Toolbar compaction">
          <DsRow label="ShellToolbar · shell-toolbar.css · DESIGN.md « Toolbar compaction »">
            <p className="ds-note" style={{ margin: 0, fontSize: "var(--fs-sm)", color: "var(--t2)", maxWidth: 520 }}>
              Barre d’instance : container queries. ≤940&nbsp;px — counts icône+chiffre
              (tip), select → icône+r.N, boutons libellés → icône seule (réf. IconButton ;
              type inchangé). ≤700&nbsp;px — stats lead masquées. Borders = actions
              principales rares seulement (select / CTA slot + « Ajouter un mod »).
            </p>
          </DsRow>
          <DsRow label="Slot action principale · DESIGN.md « Slot action principale »">
            <p className="ds-note" style={{ margin: 0, fontSize: "var(--fs-sm)", color: "var(--t2)", maxWidth: 520 }}>
              Même emplacement / hauteur que le select branche (<code>--ctl-sm</code>).
              État normal = select ; loader à installer = bouton « Installer … »
              (libellé conservé en compaction) ; après OK = select.
            </p>
            <div style={{ display: "inline-flex", alignItems: "center", gap: 12, marginTop: 8 }}>
              <Button variant="action" className="tbar-primary-action" title="Installer NeoForge">
                <Ico name="download" className="sm" />
                <span className="btn-label">Installer NeoForge</span>
              </Button>
            </div>
          </DsRow>
          <DsRow label="Exemple · action large → icône (même Button, .btn-label masqué)">
            <div style={{ display: "inline-flex", alignItems: "center", gap: 12 }}>
              <Button variant="action" title="Ajouter un mod">
                <Ico name="plus" className="sm" />
                <span className="btn-label">Ajouter un mod</span>
              </Button>
              <Button
                variant="action"
                title="Ajouter un mod"
                aria-label="Ajouter un mod"
                style={{ width: "var(--ctl-sm)", height: "var(--ctl-sm)", padding: 0, gap: 0, justifyContent: "center" }}
              >
                <Ico name="plus" className="sm" />
              </Button>
              <IconButton name="folder" title="Réf. IconButton" />
            </div>
          </DsRow>
        </DsSection>

        <DsSection title="Button">
          {buttonVariants.map((variant) => {
            const isRound = variant === "round" || variant === "ghost-round";
            return (
              <DsRow
                key={variant}
                label={`components/ui/atoms/Button.tsx · Button · ${variant}`}
              >
                <Button
                  variant={variant === "default" ? undefined : variant}
                  title={
                    variant === "default"
                      ? "Tip au survol (data-tip)"
                      : isRound
                        ? variant
                        : undefined
                  }
                  aria-label={isRound ? variant : undefined}
                >
                  {isRound ? <Ico name="plus" /> : "Label"}
                </Button>
              </DsRow>
            );
          })}
        </DsSection>

        <DsSection title="IconButton">
          <DsRow label="components/ui/atoms/IconButton.tsx · IconButton · size=md">
            <IconButton name="box" title="md" size="md" />
          </DsRow>
          <DsRow label="components/ui/atoms/IconButton.tsx · IconButton · size=sm">
            <IconButton name="box" title="sm" size="sm" />
          </DsRow>
          <DsRow label="components/ui/atoms/IconButton.tsx · IconButton · variant=sm">
            <IconButton name="box" title="sm" variant="sm" />
          </DsRow>
          <DsRow label="components/ui/atoms/IconButton.tsx · IconButton · variant=ghost">
            <IconButton name="box" title="ghost" variant="ghost" />
          </DsRow>
          <DsRow label="components/ui/atoms/IconButton.tsx · IconButton · variant=sm-ghost">
            <IconButton name="box" title="sm-ghost" variant="sm-ghost" />
          </DsRow>
        </DsSection>

        <DsSection title="Badge">
          <p className="ds-section-note">
            Flags / table. Même soft dark que Chip (16 % dans `--c1`, fg 68 % vers blanc) — voir
            Couleurs → Soft et DESIGN.md. Pas de pastille pastel claire.
          </p>
          {badgeTones.map((tone) => (
            <DsRow key={tone} label={`Badge.tsx · Badge · tone=${tone}`}>
              <Badge tone={tone} data-tip={tone} aria-label={tone}>
                <Ico name={BADGE_TONE_ICO[tone]} />
              </Badge>
            </DsRow>
          ))}
          <DsRow label="Badge.tsx · Badge · variant=ghost">
            <Badge variant="ghost" data-tip="ghost" aria-label="ghost">
              <Ico name="map" />
            </Badge>
          </DsRow>
        </DsSection>

        <DsSection title="Chip">
          <p className="ds-section-note">
            Modes composer + tags `@mod`. Soft dark partagé avec Badge — voir DESIGN.md.
          </p>
          {(
            [
              { tone: "plan", label: "Plan" },
              { tone: "debug", label: "Debug" },
              { tone: "multi", label: "Multi-tâches" },
              { tone: "mod", label: "@SSE Engine Fixes" },
            ] as const satisfies readonly { tone: ChipTone; label: string }[]
          ).map(({ tone, label }) => (
            <DsRow key={tone} label={`Chip.tsx · Chip · tone=${tone}`}>
              <Chip tone={tone} label={label} onDismiss={() => undefined} />
            </DsRow>
          ))}
        </DsSection>

        <DsSection title="Switch">
          <DsRow label="components/ui/atoms/Switch.tsx · Switch · checked">
            <Switch checked={swOn} onToggle={() => setSwOn((v) => !v)} />
          </DsRow>
          <DsRow label="components/ui/atoms/Switch.tsx · Switch · disabled">
            <Switch checked={false} disabled onToggle={() => undefined} />
          </DsRow>
        </DsSection>

        <DsSection title="Input">
          {inputVariants.map((variant) => (
            <DsRow key={variant} label={`components/ui/atoms/Input.tsx · Input · ${variant}`}>
              <Input variant={variant} placeholder="Texte…" defaultValue="" />
            </DsRow>
          ))}
        </DsSection>

        <DsSection title="Select">
          <p className="ds-section-note">
            Référence produit = select branche de la toolbar Mods (
            <code>ShellToolbar</code>). Atome <code>Select</code> = déclencheur bordé (
            icône + libellé + méta optionnelle + chevron, hauteur <code>--ctl-sm</code>,
            bordure action). Le panneau = <code>PopoverMenu</code> — pas un{" "}
            <code>&lt;select&gt;</code> natif. Voir DESIGN.md « Select ».
          </p>
          <DsRow label="components/ui/atoms/Select.tsx · Select · déclencheur">
            <Select
              icon="branch"
              label={selectCurrent.label}
              meta={selectCurrent.meta}
              title={`${selectCurrent.label} · ${selectCurrent.meta}`}
              aria-label={`Branche ${selectCurrent.label}`}
            />
          </DsRow>
          <DsRow label="Select + PopoverMenu · pattern complet (réf. toolbar)">
            <div style={{ position: "relative", display: "inline-flex" }}>
              <Select
                ref={selectRef}
                icon="branch"
                label={selectCurrent.label}
                meta={selectCurrent.meta}
                title={`${selectCurrent.label} · ${selectCurrent.meta}`}
                aria-haspopup="dialog"
                aria-expanded={selectOpen}
                aria-label={`Branche ${selectCurrent.label}`}
                onClick={() => setSelectOpen((v) => !v)}
              />
              {selectOpen ? (
                <PopoverMenu
                  inline
                  title="Changer de branche"
                  anchorRef={selectRef}
                  onClose={() => setSelectOpen(false)}
                >
                  {selectOptions.map((opt) => (
                    <PopoverMenuItem
                      key={opt.id}
                      icon="branch"
                      label={opt.label}
                      sub={opt.meta}
                      current={opt.id === selectValue}
                      badge={opt.id === selectValue ? "active" : undefined}
                      onPick={() => {
                        setSelectValue(opt.id);
                        setSelectOpen(false);
                      }}
                    />
                  ))}
                </PopoverMenu>
              ) : null}
            </div>
          </DsRow>
        </DsSection>

        <DsSection title="Checkbox">
          <DsRow label="components/ui/atoms/Checkbox.tsx · Checkbox · default">
            <Checkbox checked={chkOn} onToggle={() => setChkOn((v) => !v)} />
          </DsRow>
          <DsRow label="components/ui/atoms/Checkbox.tsx · Checkbox · mod">
            <Checkbox
              variant="mod"
              checked={chkModOn}
              onToggle={() => setChkModOn((v) => !v)}
            />
          </DsRow>
        </DsSection>

        <DsSection title="Radio">
          <DsRow label="components/ui/atoms/Radio.tsx · Radio">
            <Radio name="ds-radio" checked={radioOn} onSelect={() => setRadioOn(true)} />
            <Radio name="ds-radio" checked={!radioOn} onSelect={() => setRadioOn(false)} />
          </DsRow>
        </DsSection>

        <DsSection title="ChannelDot">
          {channels.map((channel) => (
            <DsRow
              key={channel}
              label={`components/ui/atoms/ChannelDot.tsx · ChannelDot · ${channel}`}
            >
              <ChannelDot channel={channel} />
            </DsRow>
          ))}
        </DsSection>

        <DsSection title="SearchInput">
          <DsRow label="components/ui/molecules/SearchInput.tsx · SearchInput · compact=false">
            <SearchInput value={search} onChange={setSearch} placeholder="Rechercher…" />
          </DsRow>
          <DsRow label="components/ui/molecules/SearchInput.tsx · SearchInput · compact=true">
            <SearchInput value={search} onChange={setSearch} placeholder="Rechercher…" compact />
          </DsRow>
        </DsSection>

        <DsSection title="PopoverMenu">
          <DsRow label="PopoverMenu · chrome=context (sans titre)">
            <PopoverMenu inline chrome="context" onClose={() => undefined}>
              <PopoverMenuItem label="Couper" shortcut="Ctrl+X" onPick={() => undefined} />
              <PopoverMenuItem label="Copier" shortcut="Ctrl+C" onPick={() => undefined} />
              <PopoverMenuItem label="Coller" shortcut="Ctrl+V" onPick={() => undefined} />
              <PopoverMenuSep />
              <PopoverMenuItem label="Tout sélectionner" shortcut="Ctrl+A" onPick={() => undefined} />
            </PopoverMenu>
          </DsRow>
          <DsRow label="PopoverMenu · chrome=context (icônes + sous-menu)">
            <PopoverMenu inline chrome="context" onClose={() => undefined}>
              <PopoverMenuEntries
                items={[
                  { id: "pin", label: "Épingler", icon: "star" },
                  { id: "rename", label: "Renommer", icon: "edit" },
                  {
                    id: "copy",
                    label: "Copier",
                    icon: "copy",
                    children: [
                      { id: "copy-id", label: "Copier l'identifiant" },
                      { id: "copy-path", label: "Copier le chemin" },
                      { id: "copy-link", label: "Copier le lien" },
                    ],
                  },
                  { id: "archive", label: "Archiver", icon: "box", shortcut: "Ctrl+Shift+E" },
                ]}
                onSelect={() => undefined}
              />
            </PopoverMenu>
          </DsRow>
          <DsRow label="PopoverMenu · chrome=dialog (titre + croix)">
            <PopoverMenu
              inline
              chrome="dialog"
              title="Changer de branche"
              onClose={() => undefined}
              utility={{
                kind: "search",
                value: search,
                onChange: setSearch,
                placeholder: "Trouver une branche…",
              }}
              tabs={[
                { id: "branches", label: "Branches" },
                { id: "revisions", label: "Révisions" },
              ]}
              activeTab="branches"
              onTabChange={() => undefined}
              footer={
                <PopoverMenuFooterLink onClick={() => undefined}>
                  Voir toutes les branches
                </PopoverMenuFooterLink>
              }
            >
              <PopoverMenuItem
                icon="branch"
                label="principale"
                sub="Stable · r.12"
                current
                badge="active"
                onPick={() => undefined}
              />
              <PopoverMenuItem
                icon="branch"
                label="essai"
                sub="Beta · b.1"
                onPick={() => undefined}
              />
              <PopoverMenuSep />
              <PopoverMenuItem
                icon="history"
                label="r.11"
                sub="instantané passé"
                onPick={() => undefined}
              />
            </PopoverMenu>
          </DsRow>
        </DsSection>

        <DsSection title="Toast">
          <DsRow label="components/ui/molecules/Toast.tsx · Toast">
            <Toast icon="check" style={{ position: "relative", transform: "none", bottom: "auto" }}>
              Message exemple
            </Toast>
          </DsRow>
        </DsSection>

        <DsSection title="SideEmpty">
          <DsRow label="components/ui/molecules/SideEmpty.tsx · SideEmpty">
            <SideEmpty
              title="Rien ici"
              action={
                <Button variant="sm-ghost" type="button">
                  Action
                </Button>
              }
            >
              Aucun élément pour l’instant.
            </SideEmpty>
          </DsRow>
        </DsSection>

        <DsSection title="FavoriteButton">
          <DsRow label="components/ui/atoms/FavoriteButton.tsx · FavoriteButton · on=false">
            <FavoriteButton on={false} onToggle={() => undefined} />
          </DsRow>
          <DsRow label="components/ui/atoms/FavoriteButton.tsx · FavoriteButton · on=true">
            <FavoriteButton on onToggle={() => undefined} />
          </DsRow>
          <DsRow label="components/ui/atoms/FavoriteButton.tsx · FavoriteButton · interactif">
            <FavoriteButton on={favOn} onToggle={() => setFavOn((v) => !v)} />
          </DsRow>
        </DsSection>

        <DsSection title="PixelLoader">
          <DsRow label="components/ui/atoms/PixelLoader.tsx · PixelLoader">
            <PixelLoader />
          </DsRow>
        </DsSection>

        <DsSection title="SettingsRow">
          <DsRow label="components/ui/molecules/SettingsRow.tsx · SettingsRow">
            <SettingsRow title="Titre" desc="Description" control={<Badge tone="root">badge</Badge>} />
          </DsRow>
          <DsRow label="components/ui/molecules/SettingsRow.tsx · SettingsRow · indent">
            <SettingsRow title="Titre indenté" indent />
          </DsRow>
        </DsSection>

        <DsSection title="SideBack">
          <DsRow label="components/ui/molecules/SideBack.tsx · SideBack">
            <SideBack label="Retour" onClick={() => undefined} />
          </DsRow>
        </DsSection>

        <DsSection title="SideRow">
          <DsRow label="components/ui/molecules/SideRow.tsx · SideRow">
            <div style={{ width: "100%" }}>
              <SideRow icon="box" label="Ligne" onClick={() => undefined} />
            </div>
          </DsRow>
          <DsRow label="components/ui/molecules/SideRow.tsx · SideRow · current">
            <div style={{ width: "100%" }}>
              <SideRow icon="box" label="Actuelle" current onClick={() => undefined} />
            </div>
          </DsRow>
        </DsSection>

        <DsSection title="Crumbs">
          <DsRow label="components/ui/molecules/Crumbs.tsx · Crumbs">
            <Crumbs path={["Racine", "Section", "Page"]} tail="F:\\ModLab" />
          </DsRow>
        </DsSection>

        <DsSection title="Panel">
          <DsRow label="components/ui/organisms/Panel.tsx · Panel">
            <Panel icon="info" title="Titre du panneau" bodyClassName="why">
              Corps du panneau.
            </Panel>
          </DsRow>
        </DsSection>

        <DsSection title="Accordion">
          <DsRow label="components/ui/organisms/Accordion.tsx · Accordion">
            <Accordion title="Section" count={3}>
              <p style={{ margin: 0, fontSize: "var(--fs-sm)", color: "var(--t2)" }}>Contenu repliable.</p>
            </Accordion>
          </DsRow>
          <DsRow label="components/ui/organisms/Accordion.tsx · Accordion · variant=rsec">
            <Accordion title="Inspector" count={1} variant="rsec" defaultOpen={false}>
              <p style={{ margin: 0, fontSize: "var(--fs-sm)", color: "var(--t2)" }}>Variante rsec.</p>
            </Accordion>
          </DsRow>
        </DsSection>

        <DsSection title="Dialog">
          <DsRow label="components/ui/molecules/Dialog.tsx · Dialog">
            <Button variant="sm-ghost" onClick={() => setDialogOpen(true)}>
              Ouvrir
            </Button>
            {dialogOpen ? (
              <Dialog titleId="ds-dialog" title="Dialogue" onClose={() => setDialogOpen(false)}>
                <p style={{ margin: 0 }}>Corps minimal.</p>
              </Dialog>
            ) : null}
          </DsRow>
        </DsSection>

        <DsSection title="ConfirmDialog">
          <DsRow label="components/ui/molecules/ConfirmDialog.tsx · ConfirmDialog">
            <Button variant="sm-ghost" onClick={() => setConfirmOpen(true)}>
              Ouvrir
            </Button>
            {confirmOpen ? (
              <ConfirmDialog
                title="Confirmer ?"
                body="Action réversible."
                onConfirm={() => undefined}
                onClose={() => setConfirmOpen(false)}
              />
            ) : null}
          </DsRow>
          <DsRow label="components/ui/molecules/ConfirmDialog.tsx · ConfirmDialog · danger">
            <Button variant="sm-ghost" onClick={() => setConfirmDangerOpen(true)}>
              Ouvrir (danger)
            </Button>
            {confirmDangerOpen ? (
              <ConfirmDialog
                title="Action destructive"
                body="Irréversible."
                danger
                confirmLabel="Supprimer"
                onConfirm={() => undefined}
                onClose={() => setConfirmDangerOpen(false)}
              />
            ) : null}
          </DsRow>
        </DsSection>
      </div>
    </div>
  );
}