# Spikes

Time-boxed, throwaway explorations for the risks flagged in `docs/01-architecture-decidee.md`. Code here is never merged into `crates/` — see `CLAUDE.md`.

## sevenz-rust — ✅ resolved, 2026-09-15

**Question:** can we extract `.7z` archives in pure Rust, without depending on `7z.dll`?

**Result:** yes. `spikes/sevenz-rust/` extracts a real SKSE archive (`Skyrim Script Extender (SKSE64) Steam 2.2.8.7z`, 129 files) cleanly with the `sevenz-rust` crate, no external DLL. `.zip` is separately a solved problem (the mature `zip` crate). RAR is not yet spiked — smaller share of Nexus archives, not blocking.

**Decision:** use `sevenz-rust` + `zip` for archive handling. Revisit RAR only if it turns out to matter in practice.

## USVFS FFI — ✅ feasible, not yet implemented, 2026-09-15

**Question:** can Rust call into `usvfs_x64.dll` (C++, used by MO2) without a painful FFI layer?

**Result:** `dumpbin /EXPORTS` on Benjamin's real `usvfs_x64.dll` (from his MO2 install at `F:\_Modding\Skyrim Modding\Mod Organazer\`) shows two families of exports: internal C++-mangled symbols (not directly callable), and a **clean, unmangled C ABI** — `usvfsCreateVFS`, `usvfsConnectVFS`, `usvfsCreateParameters`, `usvfsCreateHookContext`, `CreateVFS`, `ConnectVFS`, `InitHooks`, `InitLogging`, etc. This is a real, intentional C API surface, not just internals leaking out.

**Decision:** FFI via `extern "C"` + either `libloading` (dynamic, no link-time dependency) or a generated import lib is viable. Full binding (matching real function signatures to MO2's usage of them) is real work for a later Phase 1 task, not done here — this spike only proves it's *possible*, which was the actual risk.

## MCP Rust SDK — ✅ resolved, 2026-09-15

**Question:** is there a Rust MCP SDK mature enough to match the official C# one?

**Result:** yes — **`rmcp`**, the official Rust SDK at `github.com/modelcontextprotocol/rust-sdk`, implements the current MCP spec (2026-07-28) with a tokio async runtime. `rust-mcp-sdk` (community) is a solid alternative, passing the full official conformance suite. No need for a wrapper or fallback.

**Decision:** use `rmcp`.

## llama.cpp Rust binding — ✅ resolved, 2026-09-15

**Question:** `llama-cpp-2` (thin binding, closer to LLamaSharp's role) vs `candle` (pure-Rust reimplementation)?

**Result:** `llama-cpp-2` compiles and links cleanly, full C++ llama.cpp build included (~2 min from scratch). Needed two extra machine-level dependencies neither of which were present: **LLVM** (for `libclang.dll`, which `bindgen` needs to generate the FFI layer from llama.cpp's C headers) and **CMake** (to actually build llama.cpp). Both installed via winget. `spikes/llama-binding/` runs.

**Decision:** use `llama-cpp-2`. Closest analog to the already-proven LLamaSharp approach (GGUF, CPU/CUDA/Vulkan backends, thin wrapper over the real llama.cpp) — no need to fall back to `candle`.

⚠️ **Consequence for packaging later:** whatever machine actually builds ModLab's release binaries needs LLVM + CMake installed too (dev-time only — these aren't runtime dependencies of the shipped app, `llama-cpp-sys-2` statically links what it builds). Worth a line in the eventual CI/build docs.

## xEdit orchestration — 🚧 blocked, no binary on this machine, 2026-09-15

xEdit/SSEEdit isn't installed standalone on Benjamin's machine (checked `F:\_Modding\Skyrim Modding\` and common tool locations — LOOT and MO2 are there, xEdit isn't). The orchestration approach itself (CLI args: `-autoload -autoexit -veryquickshowconflicts` + Pascal scripts, capture output) doesn't need a spike to *design* — it's the same pattern MO2 already uses — but it can't be *verified* end-to-end without the binary. Needs Benjamin to grab xEdit (or point at wherever he keeps it) before this can move past design.
