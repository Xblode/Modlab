//! FFI binding to `usvfs` (`usvfs_x64.dll`) — the C++ virtual filesystem
//! engine MO2 uses to deploy mods without touching the real `Data/` folder.
//! See `docs/01-architecture-decidee.md` §4/§8 and `spikes/README.md`'s
//! "USVFS FFI" entry: that spike proved the DLL exports a clean, unmangled C
//! ABI (`dumpbin /EXPORTS` on Benjamin's real `usvfs_x64.dll`); this module
//! is the real binding built on top of that.
//!
//! Function signatures and the opaque `usvfsParameters` type come from
//! usvfs's real public headers (`include/usvfs/usvfs.h` and
//! `usvfsparameters.h` in the `ModOrganizer2/usvfs` GitHub repo) — but the
//! header is from current `master`, and Benjamin's real `usvfs_x64.dll`
//! (bundled with his MO2 install) is an older build (`0.5.5.1`) mid-way
//! through a renaming migration: `dumpbin /EXPORTS` shows the *parameter*
//! functions already using the modern `usvfs`-prefixed names
//! (`usvfsCreateParameters`, `usvfsSetInstanceName`, `usvfsCreateVFS`,
//! `usvfsFreeParameters`), but the actual VFS operations still only exist
//! under their old, unprefixed names: `VirtualLinkDirectoryStatic`,
//! `CreateProcessHooked`, `DisconnectVFS`. There is no `usvfsVersionString`
//! at all in this build — only an old-style `USVFSVersionString`, whose
//! calling convention isn't documented anywhere we have; rather than guess
//! a C++ ABI detail that could corrupt the stack if wrong, this binding
//! simply doesn't call it. Never assume a symbol name from the header
//! without checking it's actually exported by the DLL being loaded — that's
//! what caused the very first version of this module to fail immediately
//! with `GetProcAddress` "procedure not found."
//!
//! Loaded dynamically via `libloading`, not at link time — per the spike's
//! decision, and because ModLab doesn't vendor its own copy of the DLL yet.
//! Callers supply whatever path they have (Benjamin's real MO2 install
//! during Phase 1 dev).
//!
//! What "deployment" means here: `usvfs` doesn't copy or move a single real
//! file. It hooks Windows file APIs *inside one specific child process* so
//! that process sees a merged view of several real directories layered on
//! top of each other. Nothing on disk changes. The actual game/SKSE launch
//! (a separate, still-unapproved checkpoint) is just "spawn that child
//! process for real" instead of a read-only verification command.

use std::ffi::{c_void, CString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use libloading::{Library, Symbol};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, WaitForSingleObject, CREATE_NO_WINDOW, PROCESS_INFORMATION, STARTUPINFOW,
};

const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 0x0000_0102;

/// `LINKFLAG_RECURSIVE` from `usvfs.h` — link a directory's contents
/// recursively rather than just its immediate children.
const LINKFLAG_RECURSIVE: u32 = 0x0000_0008;

#[derive(Debug, thiserror::Error)]
pub enum UsvfsError {
    #[error("failed to load {0}: {1}")]
    LoadLibrary(PathBuf, libloading::Error),
    #[error("symbol missing from usvfs_x64.dll: {0}")]
    Symbol(#[from] libloading::Error),
    #[error("instance name contains a nul byte: {0:?}")]
    InvalidInstanceName(String),
    #[error("usvfsCreateParameters returned null")]
    CreateParametersFailed,
    #[error("usvfsCreateVFS reported failure for instance '{0}'")]
    CreateVfsFailed(String),
    #[error("usvfsVirtualLinkDirectoryStatic reported failure linking {from} onto {onto}")]
    LinkDirectoryFailed { from: PathBuf, onto: PathBuf },
    #[error("usvfsCreateProcessHooked reported failure launching: {0}")]
    CreateProcessHookedFailed(String),
    #[error("hooked process did not exit within the timeout: {0}")]
    ProcessTimedOut(String),
    #[error("GetVFSProcessList reported failure")]
    GetProcessListFailed,
    #[error("no process ever registered with the VFS — the launch likely failed silently")]
    NeverRegistered,
    #[error(
        "usvfs instance '{instance}' disconnect could not be confirmed — {remaining} process(es) still registered"
    )]
    DisconnectNotConfirmed { instance: String, remaining: usize },
}

/// Deterministic, stable VFS instance name for a given game + loadout.
///
/// Two different loadouts — even for the same game — must never share a VFS
/// instance name: `usvfs` instances are named, machine-wide, shared-memory
/// resources (see `Vfs`'s doc), so a shared name means two independently
/// "isolated" mod setups actually see each other's virtual file mappings.
/// `loadout_id` is the SQLite primary key backing every loadout, so it's
/// already globally unique and immutable — deriving the name from it means
/// relaunching the *same* loadout always reconnects under the *same*
/// identity (useful for recovering a stale session left by a crash, see
/// `Vfs::create`) while two different loadouts can never collide. The
/// `modlab-` prefix namespaces this away from unrelated tools (e.g. MO2's
/// own instance, if MO2 is open at the same time).
#[must_use]
pub fn instance_name(game_id: &str, loadout_id: i64) -> String {
    format!("modlab-{game_id}-{loadout_id}")
}

/// Opaque handle — `usvfs.h` forward-declares `struct usvfsParameters;` and
/// never exposes its layout, only pointers to it. Rust mirrors that: we
/// never read or write its fields, only pass the pointer back to the DLL.
#[repr(C)]
struct RawUsvfsParameters {
    _private: [u8; 0],
}

type FnCreateParameters = unsafe extern "system" fn() -> *mut RawUsvfsParameters;
type FnFreeParameters = unsafe extern "system" fn(*mut RawUsvfsParameters);
type FnSetInstanceName =
    unsafe extern "system" fn(*mut RawUsvfsParameters, *const std::os::raw::c_char);
type FnCreateVfs = unsafe extern "system" fn(*const RawUsvfsParameters) -> i32;
type FnDisconnectVfs = unsafe extern "system" fn();
type FnVirtualLinkDirectoryStatic = unsafe extern "system" fn(*const u16, *const u16, u32) -> i32;
type FnCreateProcessHooked = unsafe extern "system" fn(
    *const u16,               // lpApplicationName
    *mut u16,                 // lpCommandLine
    *mut SECURITY_ATTRIBUTES, // lpProcessAttributes
    *mut SECURITY_ATTRIBUTES, // lpThreadAttributes
    i32,                      // bInheritHandles (BOOL)
    u32,                      // dwCreationFlags
    *const c_void,            // lpEnvironment
    *const u16,               // lpCurrentDirectory
    *mut STARTUPINFOW,        // lpStartupInfo
    *mut PROCESS_INFORMATION, // lpProcessInformation
) -> i32;
/// `BOOL GetVFSProcessList(size_t* count, LPDWORD processIDs)` — `count` is
/// both the caller's buffer capacity going in and the actual number of
/// entries written coming out.
type FnGetVfsProcessList = unsafe extern "system" fn(*mut usize, *mut u32) -> i32;

/// A live connection to a `usvfs` virtual filesystem instance, created in
/// *this* process (the "controller") but not hooking this process itself —
/// `usvfsCreateVFS`'s own doc comment says it doesn't hook the caller, only
/// processes spawned via `run_hooked`.
///
/// **Only one `Vfs` may be alive per OS process at a time.** Confirmed for
/// real, not just documented: `usvfsCreateVFS`'s header comment says a
/// process "can only be connected to one vfs" and will silently switch to
/// the new one — and creating two `Vfs`es concurrently on two threads of
/// the *same* test binary produced a real, reproducible `CreateVfsFailed`
/// (see `VFS_TEST_LOCK` below, added after hitting exactly that flake).
/// Since `modlab-cli` is one process per invocation this is a non-issue in
/// practice, but it matters a lot for tests and for any future long-running
/// ModLab process (a Tauri backend) that might want more than one.
pub struct Vfs {
    lib: Library,
    instance_name: String,
    /// Set once `disconnect()` has confirmed no process remains registered.
    /// Lets `Drop` skip redundant teardown work for a connection that was
    /// already cleanly closed by an explicit `disconnect()` call.
    disconnected: bool,
}

impl Vfs {
    /// Load `usvfs_x64.dll` from `dll_path` and create a fresh, named VFS
    /// instance. `instance_name` must be distinct from any VFS instance
    /// already running on the machine (e.g. MO2's own, if MO2 is open) —
    /// each instance is isolated behind its own named shared-memory
    /// segment, keyed by this name. Use `instance_name()` (this module) to
    /// derive one that's guaranteed distinct per loadout.
    ///
    /// If creation fails for a name under our own `modlab-` namespace, this
    /// assumes it may be a stale session left behind by a crashed prior
    /// ModLab run (crashing skips `Drop`, so a machine-wide named VFS
    /// session can outlive the process that created it) and makes one
    /// best-effort disconnect-then-retry attempt before giving up. Never
    /// retries for a name outside our namespace — that's some other tool's
    /// instance, and blindly disconnecting it would be destructive.
    pub fn create(dll_path: &Path, instance_name: &str) -> Result<Self, UsvfsError> {
        let lib = unsafe { Library::new(dll_path) }
            .map_err(|e| UsvfsError::LoadLibrary(dll_path.to_path_buf(), e))?;

        match Self::try_create_vfs(&lib, instance_name) {
            Ok(()) => {}
            Err(UsvfsError::CreateVfsFailed(_)) if instance_name.starts_with("modlab-") => {
                unsafe {
                    if let Ok(disconnect) = lib.get::<FnDisconnectVfs>(b"DisconnectVFS\0") {
                        disconnect();
                    }
                }
                Self::try_create_vfs(&lib, instance_name)?;
            }
            Err(e) => return Err(e),
        }

        Ok(Self {
            lib,
            instance_name: instance_name.to_string(),
            disconnected: false,
        })
    }

    fn try_create_vfs(lib: &Library, instance_name: &str) -> Result<(), UsvfsError> {
        let name_c = CString::new(instance_name)
            .map_err(|_| UsvfsError::InvalidInstanceName(instance_name.to_string()))?;

        let created = unsafe {
            let create_parameters: Symbol<FnCreateParameters> =
                lib.get(b"usvfsCreateParameters\0")?;
            let params = create_parameters();
            if params.is_null() {
                return Err(UsvfsError::CreateParametersFailed);
            }

            let set_instance_name: Symbol<FnSetInstanceName> =
                lib.get(b"usvfsSetInstanceName\0")?;
            set_instance_name(params, name_c.as_ptr());

            let create_vfs: Symbol<FnCreateVfs> = lib.get(b"usvfsCreateVFS\0")?;
            let ok = create_vfs(params);

            let free_parameters: Symbol<FnFreeParameters> = lib.get(b"usvfsFreeParameters\0")?;
            free_parameters(params);

            ok
        };

        if created == 0 {
            return Err(UsvfsError::CreateVfsFailed(instance_name.to_string()));
        }
        Ok(())
    }

    /// Layer `source`'s files virtually on top of `destination`, recursively.
    /// Calling this again for a different `source` onto the *same*
    /// `destination` doesn't fail — it overlays, which is exactly what
    /// deploying several mods onto one game `Data/` folder needs. No file
    /// on real disk is read, moved, or written by this call.
    pub fn link_directory_recursive(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<(), UsvfsError> {
        let source_w = to_wide(source);
        let dest_w = to_wide(destination);

        let ok = unsafe {
            let link_fn: Symbol<FnVirtualLinkDirectoryStatic> =
                self.lib.get(b"VirtualLinkDirectoryStatic\0")?;
            link_fn(source_w.as_ptr(), dest_w.as_ptr(), LINKFLAG_RECURSIVE)
        };

        if ok == 0 {
            return Err(UsvfsError::LinkDirectoryFailed {
                from: source.to_path_buf(),
                onto: destination.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Spawn `command_line` as a child process that sees this VFS's merged
    /// view, hidden (`CREATE_NO_WINDOW`) since this is meant for short,
    /// read-only verification commands, not something a human watches.
    /// Waits up to `timeout` and returns the exit code.
    ///
    /// Deliberately doesn't wire up stdout/stderr pipes — callers that want
    /// output should have the command redirect to a file *outside* any
    /// linked directory (e.g. `cmd /c dir ... > out.txt`); that file write
    /// isn't virtualized, it's a normal disk write, so it's readable back
    /// from this process once `run_hooked` returns.
    pub fn run_hooked(&self, command_line: &str, timeout: Duration) -> Result<u32, UsvfsError> {
        let process_info = self.spawn_hooked(command_line, None, CREATE_NO_WINDOW)?;

        let wait_ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        let wait_result = unsafe { WaitForSingleObject(process_info.hProcess, wait_ms) };

        let mut exit_code = 0u32;
        unsafe {
            GetExitCodeProcess(process_info.hProcess, &mut exit_code);
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
        }

        match wait_result {
            WAIT_OBJECT_0 => Ok(exit_code),
            WAIT_TIMEOUT => Err(UsvfsError::ProcessTimedOut(command_line.to_string())),
            _ => Err(UsvfsError::ProcessTimedOut(command_line.to_string())),
        }
    }

    /// Launch `command_line` as a real, *visible* child process (no
    /// `CREATE_NO_WINDOW`) that sees this VFS's merged view — meant for an
    /// interactive program a human is about to watch (the game), not a
    /// verification command. `working_dir` matters here in a way it doesn't
    /// for `run_hooked`'s `cmd.exe` calls: `skse64_loader.exe` resolves some
    /// of its own paths relative to its current directory, so callers
    /// should pass the game's install root.
    ///
    /// Doesn't wait for exit — returns the new process's id immediately.
    /// Use `wait_for_session_end` afterwards, not `run_hooked`'s
    /// single-handle wait: `skse64_loader.exe` itself exits in seconds
    /// after handing off to the real game process, which is what actually
    /// needs the VFS to stay connected for the whole play session.
    pub fn launch_hooked_visible(
        &self,
        command_line: &str,
        working_dir: &Path,
    ) -> Result<u32, UsvfsError> {
        let process_info = self.spawn_hooked(command_line, Some(working_dir), 0)?;
        let pid = process_info.dwProcessId;
        unsafe {
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
        }
        Ok(pid)
    }

    fn spawn_hooked(
        &self,
        command_line: &str,
        working_dir: Option<&Path>,
        creation_flags: u32,
    ) -> Result<PROCESS_INFORMATION, UsvfsError> {
        let mut cmdline_w = to_wide_mut(command_line);
        let working_dir_w = working_dir.map(to_wide);
        let mut startup_info: STARTUPINFOW = unsafe { std::mem::zeroed() };
        startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

        let ok = unsafe {
            let create_process_hooked: Symbol<FnCreateProcessHooked> =
                self.lib.get(b"CreateProcessHooked\0")?;
            create_process_hooked(
                std::ptr::null(),
                cmdline_w.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                creation_flags,
                std::ptr::null(),
                working_dir_w
                    .as_ref()
                    .map_or(std::ptr::null(), |w| w.as_ptr()),
                &mut startup_info,
                &mut process_info,
            )
        };

        if ok == 0 {
            return Err(UsvfsError::CreateProcessHookedFailed(
                command_line.to_string(),
            ));
        }
        Ok(process_info)
    }

    /// How many processes are currently registered against this VFS
    /// instance — the hooked launcher, the real game it hands off to, and
    /// anything either of those spawns in turn. `GetVFSProcessList` fills a
    /// caller-provided buffer; 256 slots is far more than any real modded
    /// Skyrim session needs (loader + game + maybe a launcher helper).
    fn registered_process_count(&self) -> Result<usize, UsvfsError> {
        let mut pids = [0u32; 256];
        let mut count = pids.len();

        let ok = unsafe {
            let get_list: Symbol<FnGetVfsProcessList> = self.lib.get(b"GetVFSProcessList\0")?;
            get_list(&mut count, pids.as_mut_ptr())
        };

        if ok == 0 {
            return Err(UsvfsError::GetProcessListFailed);
        }
        Ok(count)
    }

    /// Block until no process is registered against this VFS instance any
    /// more — i.e. the game (and everything it spawned) has fully exited,
    /// not just the loader that was directly launched. Polls rather than
    /// waiting on a single process handle because `skse64_loader.exe`
    /// itself exits within seconds of handing off to the real game process,
    /// long before the play session is actually over.
    ///
    /// First waits up to `startup_timeout` for at least one process to
    /// register at all (distinguishes "still starting up" from "already
    /// exited" — without this, a check taken in the brief window right
    /// after `launch_hooked_visible` returns could read 0 and wrongly
    /// conclude the session already ended).
    pub fn wait_for_session_end(
        &self,
        poll_interval: Duration,
        startup_timeout: Duration,
    ) -> Result<(), UsvfsError> {
        let start = std::time::Instant::now();
        loop {
            if self.registered_process_count()? > 0 {
                break;
            }
            if start.elapsed() > startup_timeout {
                return Err(UsvfsError::NeverRegistered);
            }
            std::thread::sleep(poll_interval);
        }

        loop {
            std::thread::sleep(poll_interval);
            if self.registered_process_count()? == 0 {
                return Ok(());
            }
        }
    }

    #[must_use]
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// Explicitly disconnect from this VFS instance and confirm — by
    /// re-checking `GetVFSProcessList` — that no process is left registered
    /// against it. Prefer calling this over relying solely on `Drop`
    /// wherever the `Result` can actually be checked: `Drop` can't
    /// propagate an error, so a teardown that silently fails there can only
    /// ever be logged, never surfaced to a caller who might want to retry
    /// or warn the user. Safe to call more than once.
    pub fn disconnect(&mut self) -> Result<(), UsvfsError> {
        if self.disconnected {
            return Ok(());
        }

        unsafe {
            let disconnect: Symbol<FnDisconnectVfs> = self.lib.get(b"DisconnectVFS\0")?;
            disconnect();
        }

        let remaining = self.registered_process_count()?;
        if remaining == 0 {
            self.disconnected = true;
            Ok(())
        } else {
            Err(UsvfsError::DisconnectNotConfirmed {
                instance: self.instance_name.clone(),
                remaining,
            })
        }
    }
}

/// Virtually link every *enabled* entry in `entries` onto `data_dir`, in the
/// order given. `loadout::entries` already returns rows in `position`
/// ascending order, which — now confirmed for real (see
/// `two_mods_overlay_onto_one_destination_last_linked_wins`) — means the
/// highest-position enabled mod wins any file conflict, consistent with
/// `modlab_core::conflicts`. Returns how many mods were linked. Doesn't
/// touch a single real file: `data_dir` itself is never written to.
///
/// Links `data_dir` onto itself *first*, before any mod, so the real
/// base-game files already sitting there (`Skyrim.esm`, the DLC `.esm`s,
/// `_ResourcePack.esl`, ...) are an explicit, guaranteed layer of the merged
/// view rather than an unverified assumption that `usvfs` passes pre-existing
/// files through for free. Every mod is linked afterwards, so a mod's file
/// still wins over a same-named base-game file — this only adds a layer
/// underneath, it doesn't change who's on top (see
/// `mod_overlay_still_wins_over_base_game_file_of_same_name`).
pub fn deploy_enabled_mods(
    vfs: &Vfs,
    entries: &[modlab_core::loadout::LoadoutEntryRow],
    data_dir: &Path,
) -> Result<usize, UsvfsError> {
    vfs.link_directory_recursive(data_dir, data_dir)?;

    let mut linked = 0;
    for entry in entries.iter().filter(|e| e.enabled) {
        vfs.link_directory_recursive(&entry.source_path, data_dir)?;
        linked += 1;
    }
    Ok(linked)
}

impl Drop for Vfs {
    fn drop(&mut self) {
        // Fallback safety net for early returns / panics that skip an
        // explicit `disconnect()` call — prefer calling `disconnect()`
        // directly wherever the `Result` can be checked, since `Drop` can't
        // propagate an error. If `disconnect()` already ran and confirmed
        // teardown, there's nothing left to do.
        if self.disconnected {
            return;
        }

        // Best-effort: if the symbol lookup itself fails there's nothing
        // more we can do from a Drop impl, and nothing on real disk was
        // ever touched regardless.
        let confirmed = unsafe {
            match self.lib.get::<FnDisconnectVfs>(b"DisconnectVFS\0") {
                Ok(disconnect) => {
                    disconnect();
                    self.registered_process_count()
                        .map(|remaining| remaining == 0)
                        .unwrap_or(false)
                }
                Err(_) => false,
            }
        };

        if !confirmed {
            eprintln!(
                "modlab: WARNING: usvfs instance '{}' disconnect on drop could not be confirmed — a stale VFS session may remain",
                self.instance_name
            );
        }
    }
}

fn to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Same encoding as `to_wide`, but for a command line string that
/// `CreateProcessW`-family functions may write back into (their `lpCommandLine`
/// parameter is `LPWSTR`, not `LPCWSTR`).
fn to_wide_mut(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    /// Only one `Vfs` may be connected per process (see `Vfs`'s doc) —
    /// `cargo test` runs tests in this file on separate threads of the
    /// *same* process by default, so every test that creates a `Vfs` must
    /// hold this for its whole duration or they race and flake.
    static VFS_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Benjamin's real MO2 install — every test here is a no-op (silently
    /// skipped) on a machine without it, same pattern used throughout this
    /// project for real-data tests (see `mo2.rs`, `installer.rs`).
    fn real_dll_path() -> PathBuf {
        PathBuf::from(r"F:\_Modding\Skyrim Modding\Mod Organazer\usvfs_x64.dll")
    }

    #[test]
    fn connects_to_the_real_dll_if_present() {
        let dll = real_dll_path();
        if !dll.is_file() {
            return;
        }

        let _guard = VFS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let vfs = Vfs::create(&dll, "modlab_test_connect").expect("should connect to usvfs");
        assert_eq!(vfs.instance_name(), "modlab_test_connect");
    }

    /// Confirmed for real, 2026-09-15: `VirtualLinkDirectoryStatic` overlay
    /// semantics are "last call wins" for a path both sources provide.
    /// Linking mod A then mod B onto the same destination and reading the
    /// shared file back through a hooked process returned mod B's content,
    /// not mod A's. This matches — and for the first time actually
    /// verifies — `modlab_core::conflicts`'s "last in the given sequence
    /// wins" assumption. It does *not* by itself resolve the separate,
    /// still-open question of which direction MO2's `modlist.txt` line
    /// order maps to real on-screen priority (see `mo2.rs`'s module doc) —
    /// that's about MO2's file format, not usvfs's linking behavior.
    #[test]
    fn two_mods_overlay_onto_one_destination_last_linked_wins() {
        let dll = real_dll_path();
        if !dll.is_file() {
            return;
        }

        let mod_a = tempfile::tempdir().unwrap();
        fs::write(mod_a.path().join("only_a.txt"), b"only in A").unwrap();
        fs::write(mod_a.path().join("shared.txt"), b"content-from-A").unwrap();

        let mod_b = tempfile::tempdir().unwrap();
        fs::write(mod_b.path().join("only_b.txt"), b"only in B").unwrap();
        fs::write(mod_b.path().join("shared.txt"), b"content-from-B").unwrap();

        let dest = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let out_file = work.path().join("out.txt");

        let _guard = VFS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let vfs = Vfs::create(&dll, "modlab_test_overlay").expect("should connect to usvfs");
        vfs.link_directory_recursive(mod_a.path(), dest.path())
            .expect("linking mod A should succeed");
        vfs.link_directory_recursive(mod_b.path(), dest.path())
            .expect("linking mod B should succeed");

        let cmd = format!(
            "cmd.exe /c type \"{}\\shared.txt\" > \"{}\" 2>&1",
            dest.path().display(),
            out_file.display()
        );
        let exit_code = vfs
            .run_hooked(&cmd, Duration::from_secs(10))
            .expect("hooked process should run");
        assert_eq!(exit_code, 0, "hooked `type` command should succeed");

        let content = fs::read_to_string(&out_file).unwrap();
        assert_eq!(
            content.trim(),
            "content-from-B",
            "last-linked mod should win the overlay"
        );
    }

    /// Sanity check for `wait_for_session_end` / `GetVFSProcessList` before
    /// ever pointing them at a real, long-running game session: launch a
    /// short, harmless process (~2s), confirm the wait returns once it's
    /// actually done — not instantly (would mean the startup-registration
    /// check is broken) and not stuck (would mean de-registration is never
    /// observed).
    #[test]
    fn wait_for_session_end_tracks_a_short_lived_hooked_process() {
        let dll = real_dll_path();
        if !dll.is_file() {
            return;
        }

        let _guard = VFS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let vfs = Vfs::create(&dll, "modlab_test_session").expect("should connect to usvfs");
        let work = tempfile::tempdir().unwrap();

        let started = std::time::Instant::now();
        vfs.launch_hooked_visible(r"cmd.exe /c ping -n 3 127.0.0.1 >NUL", work.path())
            .expect("should launch the hooked process");
        vfs.wait_for_session_end(Duration::from_millis(150), Duration::from_secs(5))
            .expect("session should be detected as ended");
        let elapsed = started.elapsed();

        assert!(
            elapsed >= Duration::from_millis(500),
            "returned too fast ({elapsed:?}) — startup-registration check may be broken"
        );
        assert!(
            elapsed < Duration::from_secs(15),
            "took too long ({elapsed:?}) — de-registration may never be observed"
        );
    }

    #[test]
    fn instance_name_is_deterministic_and_distinct_per_loadout() {
        assert_eq!(
            instance_name("skyrim", 42),
            instance_name("skyrim", 42),
            "same game + loadout must always derive the same name"
        );
        assert_ne!(
            instance_name("skyrim", 42),
            instance_name("skyrim", 43),
            "different loadouts of the same game must not collide"
        );
        assert_ne!(
            instance_name("skyrim", 42),
            instance_name("minecraft", 42),
            "different games sharing a loadout id must not collide"
        );
        assert!(
            instance_name("skyrim", 42).starts_with("modlab-"),
            "must live under ModLab's own namespace, distinct from other tools (e.g. MO2)"
        );
    }

    fn entry(name: &str, source_path: PathBuf, position: i64) -> modlab_core::loadout::LoadoutEntryRow {
        modlab_core::loadout::LoadoutEntryRow {
            mod_id: position,
            name: name.to_string(),
            source_path,
            enabled: true,
            position,
            version: None,
            status: modlab_core::loadout::STATUS_INSTALLED.to_string(),
            category: None,
            archive_path: None,
        }
    }

    /// Regression for the reported bug: installing SKSE on one loadout, then
    /// creating a second, fresh loadout, must NOT show SKSE (or anything
    /// else) as already present — two loadouts must never share VFS state.
    #[test]
    fn two_instances_for_different_loadouts_do_not_share_vfs_state() {
        let dll = real_dll_path();
        if !dll.is_file() {
            return;
        }

        let skse_mod = tempfile::tempdir().unwrap();
        fs::write(skse_mod.path().join("skse64_loader.exe"), b"fake skse").unwrap();

        let _guard = VFS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Loadout 1: deploy SKSE into its own destination, then disconnect —
        // simulating "SKSE installed on instance A".
        let dest_a = tempfile::tempdir().unwrap();
        {
            let mut vfs_a =
                Vfs::create(&dll, &instance_name("skyrim", 1)).expect("should connect (loadout 1)");
            vfs_a
                .link_directory_recursive(skse_mod.path(), dest_a.path())
                .expect("linking SKSE into loadout 1 should succeed");
            vfs_a.disconnect().expect("loadout 1 should disconnect cleanly");
        }

        // Loadout 2: a brand new, empty destination — must stay vanilla.
        let dest_b = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let out_file = work.path().join("out.txt");
        let vfs_b =
            Vfs::create(&dll, &instance_name("skyrim", 2)).expect("should connect (loadout 2)");
        let cmd = format!(
            "cmd.exe /c dir /b \"{}\" > \"{}\" 2>&1",
            dest_b.path().display(),
            out_file.display()
        );
        vfs_b
            .run_hooked(&cmd, Duration::from_secs(10))
            .expect("hooked dir should run");

        let listing = fs::read_to_string(&out_file).unwrap();
        assert!(
            !listing.contains("skse64_loader.exe"),
            "a fresh loadout must not see another loadout's mods: got {listing:?}"
        );
    }

    /// Best-effort simulation, not a true crash test: a genuine process
    /// crash (`TerminateProcess`, power loss) skips `Drop` entirely, which
    /// this can't reproduce from inside a normal test. What it does confirm
    /// is that `Vfs::create`'s disconnect-then-retry path (for our own
    /// `modlab-` namespace) lets the *same* instance name be recreated
    /// rather than getting permanently stuck.
    #[test]
    fn stale_instance_name_after_ungraceful_drop_can_be_recreated() {
        let dll = real_dll_path();
        if !dll.is_file() {
            return;
        }

        let _guard = VFS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let name = instance_name("skyrim", 999);

        {
            let _vfs = Vfs::create(&dll, &name).expect("first connect should succeed");
            // Deliberately no explicit disconnect() — falls out of scope and
            // only `Drop`'s best-effort teardown runs.
        }

        let _vfs2 = Vfs::create(&dll, &name)
            .expect("recreating the same instance name after a Drop-only teardown must not get permanently stuck");
    }

    /// Regression for the reported bug: after deploying mods, loading an
    /// existing save reported every DLC (and even Skyrim.esm itself) as
    /// missing. Base-game files that already exist at `data_dir` must be
    /// part of the merged view even though no mod provides them.
    #[test]
    fn base_game_files_survive_mod_overlay() {
        let dll = real_dll_path();
        if !dll.is_file() {
            return;
        }

        let dest = tempfile::tempdir().unwrap();
        for base_file in ["Skyrim.esm", "Update.esm", "Dawnguard.esm", "Dragonborn.esm"] {
            fs::write(dest.path().join(base_file), b"base game placeholder").unwrap();
        }

        let skyui = tempfile::tempdir().unwrap();
        fs::write(skyui.path().join("SkyUI.esp"), b"skyui placeholder").unwrap();

        let entries = vec![entry("SkyUI", skyui.path().to_path_buf(), 1)];
        let work = tempfile::tempdir().unwrap();
        let listing_file = work.path().join("listing.txt");
        let base_readback = work.path().join("skyrim_esm.txt");

        let _guard = VFS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let vfs = Vfs::create(&dll, "modlab_test_base_layer").expect("should connect to usvfs");
        deploy_enabled_mods(&vfs, &entries, dest.path()).expect("deploy should succeed");

        let list_cmd = format!(
            "cmd.exe /c dir /b \"{}\" > \"{}\" 2>&1",
            dest.path().display(),
            listing_file.display()
        );
        vfs.run_hooked(&list_cmd, Duration::from_secs(10))
            .expect("hooked dir should run");
        let listing = fs::read_to_string(&listing_file).unwrap();
        for expected in ["Skyrim.esm", "Update.esm", "Dawnguard.esm", "Dragonborn.esm", "SkyUI.esp"] {
            assert!(
                listing.contains(expected),
                "merged view is missing {expected}: got {listing:?}"
            );
        }

        let read_cmd = format!(
            "cmd.exe /c type \"{}\\Skyrim.esm\" > \"{}\" 2>&1",
            dest.path().display(),
            base_readback.display()
        );
        vfs.run_hooked(&read_cmd, Duration::from_secs(10))
            .expect("hooked type should run");
        let content = fs::read_to_string(&base_readback).unwrap();
        assert_eq!(
            content.trim(),
            "base game placeholder",
            "base-game file must be present with real content, not just an empty listing entry"
        );
    }

    /// The new base layer (data_dir linked onto itself) must not invert
    /// existing overlay priority: a mod's file still wins over a
    /// same-named base-game file.
    #[test]
    fn mod_overlay_still_wins_over_base_game_file_of_same_name() {
        let dll = real_dll_path();
        if !dll.is_file() {
            return;
        }

        let dest = tempfile::tempdir().unwrap();
        fs::write(dest.path().join("Skyrim.esm"), b"real-base-game").unwrap();

        let patch_mod = tempfile::tempdir().unwrap();
        fs::write(patch_mod.path().join("Skyrim.esm"), b"patched-by-mod").unwrap();

        let entries = vec![entry("Patch", patch_mod.path().to_path_buf(), 1)];
        let work = tempfile::tempdir().unwrap();
        let out_file = work.path().join("out.txt");

        let _guard = VFS_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let vfs = Vfs::create(&dll, "modlab_test_base_priority").expect("should connect to usvfs");
        deploy_enabled_mods(&vfs, &entries, dest.path()).expect("deploy should succeed");

        let cmd = format!(
            "cmd.exe /c type \"{}\\Skyrim.esm\" > \"{}\" 2>&1",
            dest.path().display(),
            out_file.display()
        );
        vfs.run_hooked(&cmd, Duration::from_secs(10))
            .expect("hooked type should run");

        let content = fs::read_to_string(&out_file).unwrap();
        assert_eq!(
            content.trim(),
            "patched-by-mod",
            "a mod's file must still win over a same-named base-game file"
        );
    }
}
