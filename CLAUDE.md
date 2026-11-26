# ModLab

Personal, Skyrim-first, multi-game mod manager — positioned as "Cursor for game modding," not a prettier MO2. Long-term project, built with Claude Code as a native teammate via MCP.

## Source of truth — read before doing anything structural

1. **`docs/01-architecture-decidee.md`** — the locked architecture. Supersedes everything in `docs/archive/` on structural decisions. Archived files are **human-only history** — **do not open or read `docs/archive/**` unless the user explicitly asks**. Never implement archive docs; if anything disagrees with `01-architecture-decidee.md`, the locked doc wins.
2. **`ui/`** (styles in `ui/src/shell.css`) — the visual reference for the UI since 2026-11-24. The old HTML mockup was removed, replaced by `ui/`. Don't redesign on the fly — visual changes go through the user first.
3. **`.bframe/`** — project steering memory: state, decisions, ideas, tasks (`ETAT.md`, `DECISIONS.md`, `IDEES.md`, `TACHES.md`). Read `.bframe/ETAT.md` first. Parked ideas live in `.bframe/IDEES.md`. After any material progress (worker delivery, structural fix), the orchestrator **updates `.bframe/ETAT.md` in the same turn** (subject line + `prochaine_action` if needed) — do not duplicate project state in chat.
4. **`docs/faisabilite.md`** — feasibility notes (the A-parts of the former `idees.md`).
5. **`docs/README.md`** — map of `docs/`.

### Code entry doors (read these instead of wandering the tree)

- **Core:** `crates/modlab-core/README.md`
- **Minecraft adapter:** `crates/modlab-game-minecraft/README.md`
- **UI shell:** `ui/src/shell/README.md`

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

- Never open, read, or cite `docs/archive/**` for decisions or implementation — superseded background reserved for the human on request. Never implement archive docs as if they were final; `01-architecture-decidee.md` is the answer.
- Never push to GitHub, never install/upgrade global tooling, never touch anything outside this repo or `F:\ModLab` without asking first.
- Never bypass Nexus's protections (automated downloads without Premium, scraping) — fragile and gets accounts banned, not just a policy nicety.
- Never give the AI/MCP layer direct filesystem or shell access — always `AI → MCP → ModLab permission tiers → Core`, per the 3-tier model in `01-architecture-decidee.md` §7.2.

## Team

bFrame runs the work. This conversation is the orchestrator and delegates to three workers: `worker-sonnet`, `worker-haiku`, `worker-web`. The eight specialist prompts (`pm`, `architect`, `core-dev`, `skyrim-dev`, `mcp-dev`, `ui-designer`, `frontend-dev`, `qa`) were removed on 2026-11-24. Tasks are tracked in `.bframe/TACHES.md`.
