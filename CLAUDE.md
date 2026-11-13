# ModLab

Personal, Skyrim-first, multi-game mod manager — positioned as "Cursor for game modding," not a prettier MO2. Long-term project, built with Claude Code as a native teammate via MCP.

## Source of truth — read before doing anything structural

1. **`docs/01-architecture-decidee.md`** — the locked architecture. Supersedes `docs/archive/00-cadrage-technique.md` and `docs/archive/Modlab — Document de cadrage & architecture.md` on every structural decision (stack, permissions, roadmap). Those two stay only as background/detail — **never implement either of them blindly**; if they disagree with `01-architecture-decidee.md`, the locked doc wins.
2. **`docs/design/shell-maquette.html`** — the visual source of truth for the UI. It is a single-file HTML/CSS/JS prototype, actively iterated. Match it pixel-for-pixel when building the real React UI; don't redesign on the fly.
3. **`.bframe/`** — project steering memory: state, decisions, ideas, tasks (`ETAT.md`, `DECISIONS.md`, `IDEES.md`, `TACHES.md`). Read `.bframe/ETAT.md` first. Parked ideas live in `.bframe/IDEES.md`.
4. **`docs/faisabilite.md`** — feasibility notes (the A-parts of the former `idees.md`).
5. **`docs/README.md`** — map of `docs/`.

## Stack (locked)

- **Core**: Rust. Owns all state (SQLite via a single writer process — "one engine, many clients": UI, CLI and MCP are all clients of the same Core, never invent their own state).
- **UI**: React + TypeScript + Vite, inside a **Tauri** shell (WebView2 on Windows).
- **Data root**: `F:\ModLab` (real mod data — archives, deployed mods, the SQLite DB). This repo is source code and docs only; never mix the two.
- **Known risk**: no mature Rust equivalent to Mutagen exists yet for typed Bethesda plugin record read/write. v1 plan is to orchestrate real xEdit (like MO2 does today) — see `01-architecture-decidee.md` §4 before touching anything related to plugin/record analysis.

## Conventions

- Code and comments in **English**. Docs and all UI-facing strings in **French**.
- Git: remote `origin` (GitHub) exists on purpose (2026-11-13). **Never push without an explicit ask** — this is a standing constraint, not a one-time preference.
- Tasks live in `.bframe/TACHES.md` (the only task list). Project state lives in `.bframe/ETAT.md`, not here.
- Every mutation goes through the transaction cycle: plan → approval → snapshot → apply → verify → commit (or automatic rollback on failure). Never write directly to game folders or delete anything outright — everything reversible, an internal trash instead of real deletion.
- Contract-first for anything multi-game: the Core never knows a specific game. A `GameAdapter` does. Same for mod platforms: the Core never knows Nexus/CurseForge specifically, a `Provider` does.

## Testing

Engine-first: xUnit-equivalent Rust integration tests against fixtures (a "fake Skyrim" — minimal `Data/` folder, synthetic plugins, synthetic archives), not against the real game. UI tests come later. A mod manager that loses a file is unusable — the Core's file-safety tests are non-negotiable, everything else can slip.

## Before committing

- Rust: `cargo fmt -- --check`, `cargo clippy`, then `cargo check && cargo test` — all must pass.
- Frontend: `npm install` (if dependencies changed) then `npm run build` — it must pass.
- Never commit if `cargo check` or `npm run build` fails.
- Commit messages follow Conventional Commits: `<type>(scope): description` (e.g. `feat(profile): ...`, `fix(game-adapter): ...`).

## What we never do

- Never implement `docs/archive/00-cadrage-technique.md` or `docs/archive/Modlab — Document de cadrage & architecture.md` as if they were final — they're superseded background, `01-architecture-decidee.md` is the answer.
- Never push to GitHub, never install/upgrade global tooling, never touch anything outside this repo or `F:\ModLab` without asking first.
- Never bypass Nexus's protections (automated downloads without Premium, scraping) — fragile and gets accounts banned, not just a policy nicety.
- Never give the AI/MCP layer direct filesystem or shell access — always `AI → MCP → ModLab permission tiers → Core`, per the 3-tier model in `01-architecture-decidee.md` §7.2.

## Team

Eight specialized agents live in `.claude/agents/`: `pm`, `architect`, `core-dev`, `skyrim-dev`, `mcp-dev`, `ui-designer`, `frontend-dev`, `qa`. See `01-architecture-decidee.md` §11 for the full role table. Tasks are tracked in `.bframe/TACHES.md`.
