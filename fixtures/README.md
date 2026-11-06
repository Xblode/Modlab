# Fixtures

Test data — never the real game, never `F:\ModLab`. See `CLAUDE.md` and `docs/01-architecture-decidee.md` §5 ("fake Skyrim" fixtures).

## `fake-skyrim/`

A minimal skeleton: `SkyrimSE.exe` + `Data/` with a plugin, an SKSE DLL, empty `meshes/`/`textures/` folders — enough shape for path/structure tests.

**Current limitation, deliberate:** the "plugin" and "DLL" here are plain text placeholders, not real binary formats. That's fine for anything that only cares about *paths and file presence* (store import, detection-adjacent tests). It is **not** fine once real ESP parsing exists (Phase 1+) — at that point these need to be regenerated as genuine minimal binary fixtures (via Mutagen output, or hand-built following the ESP spec), or record-level tests will pass against nonsense. Whoever builds the plugin parser should replace these, not assume they're already correct.

## `fake-minecraft/`

Same spirit as `fake-skyrim/`: a minimal `.minecraft`-shaped skeleton
(`launcher_profiles.json`, `versions/<id>/<id>.json`, `mods/`, `resourcepacks/`,
`saves/<world>/level.dat`) — enough shape for `MinecraftAdapter::detect` and
path/presence tests.

**Same limitation, deliberate:** the `.jar`/`.zip` files here are plain text
placeholders, not real zip archives — fine for path/presence tests, **not**
fine for anything that opens them as an archive. `modlab_game_minecraft`'s
own jar-metadata tests (`fabric.mod.json`/`quilt.mod.json`/`mods.toml`
extraction) therefore build real, minimal zip fixtures **at test time** in
Rust (via the `zip` crate), not as checked-in binaries here — same reasoning
as this file's own caution about the Skyrim plugin fixture: a binary format
that actually gets parsed needs a fixture regenerated against that real
format, not hand-waved as a text file with the right extension.
