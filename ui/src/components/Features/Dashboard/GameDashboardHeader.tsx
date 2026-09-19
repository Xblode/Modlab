import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import type { CSSProperties, ReactNode } from "react";
import { Ico } from "../../../Ico";
import { Avatar, Button, StatTile } from "../../ui";

export type HeaderMetadata = {
  id: string;
  content: ReactNode;
  title?: string;
};

export type HeaderStat = {
  id: string;
  label: string;
  value: ReactNode;
  icon: string;
  title?: string;
  onClick?: () => void;
};

export type HeaderAction = {
  id: string;
  label: string;
  icon: string;
  onClick: () => void;
  disabled?: boolean;
  title?: string;
};

type Props = {
  name: string;
  artwork: { art?: string; glyph: string; c1: string; c2: string };
  metadata: readonly HeaderMetadata[];
  stats: readonly HeaderStat[];
  actions: readonly HeaderAction[];
};

/**
 * Shared game-header organism. The dashboard supplies facts and capabilities;
 * this component only renders them using the Skyrim SE reference composition.
 * Missing artwork keeps the same wide footprint, never a separate square layout.
 */
export function GameDashboardHeader({ name, artwork, metadata, stats, actions }: Props) {
  const artRef = useRef<HTMLDivElement>(null);
  const infoRef = useRef<HTMLDivElement>(null);
  const style = { "--c1g": artwork.c1, "--c2g": artwork.c2 } as CSSProperties;

  // Preserve the maquette's sizing rule for every game, including glyph fallbacks.
  const syncHeroArt = useCallback(() => {
    const art = artRef.current;
    const info = infoRef.current;
    if (!art || !info) return;
    const height = info.getBoundingClientRect().height;
    if (!height) return;
    art.style.height = `${height}px`;
    art.style.width = `${Math.max(140, Math.min(340, (height * 460) / 215))}px`;
  }, []);

  useLayoutEffect(syncHeroArt);
  useEffect(() => {
    window.addEventListener("resize", syncHeroArt);
    // Also covers changes to the available space when a shell panel is toggled.
    const observer = new ResizeObserver(syncHeroArt);
    if (infoRef.current) observer.observe(infoRef.current);
    return () => {
      window.removeEventListener("resize", syncHeroArt);
      observer.disconnect();
    };
  }, [syncHeroArt]);

  return (
    <header className="ghero" style={style}>
      <div className="ghero-top">
        <Avatar variant="art" label={artwork.glyph} image={artwork.art} ref={artRef} />
        <div className="ginfo" ref={infoRef}>
          <h1><span className="fade">{name}</span></h1>
          <div className="meta">
            {metadata.map((item) => <span key={item.id} title={item.title}>{item.content}</span>)}
          </div>
          <div className="stats">
            {stats.map((stat) => (
              <StatTile
                key={stat.id}
                label={stat.label}
                value={stat.value}
                icon={stat.icon}
                title={stat.title}
                onClick={stat.onClick}
              />
            ))}
          </div>
        </div>
      </div>
      <div className="qa">
        {actions.map((action) => (
          <Button
            key={action.id}
            variant="sm-ghost"
            onClick={action.onClick}
            disabled={action.disabled}
            title={action.title}
          >
            <Ico name={action.icon} className="sm" />
            {action.label}
          </Button>
        ))}
      </div>
    </header>
  );
}
