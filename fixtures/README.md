# Fixtures

Test data — never the real game, never `F:\ModLab`. See `CLAUDE.md` and `docs/01-architecture-decidee.md` §5 ("fake Skyrim" fixtures).

## `fake-skyrim/`

A minimal skeleton: `SkyrimSE.exe` + `Data/` with a plugin, an SKSE DLL, empty `meshes/`/`textures/` folders — enough shape for path/structure tests.

**Current limitation, deliberate:** the "plugin" and "DLL" here are plain text placeholders, not real binary formats. That's fine for anything that only cares about *paths and file presence* (store import, detection-adjacent tests). It is **not** fine once real ESP parsing exists (Phase 1+) — at that point these need to be regenerated as genuine minimal binary fixtures (via Mutagen output, or hand-built following the ESP spec), or record-level tests will pass against nonsense. Whoever builds the plugin parser should replace these, not assume they're already correct.
