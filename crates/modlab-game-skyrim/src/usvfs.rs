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
}

impl Vfs {
    /// Load `usvfs_x64.dll` from `dll_path` and create a fresh, named VFS
    /// instance. `instance_name` must be distinct from any VFS instance
    /// already running on the machine (e.g. MO2's own, if MO2 is open) —
    /// each instance is isolated behind its own named shared-memory
    /// segment, keyed by this name.
    pub fn create(dll_path: &Path, instance_name: &str) -> Result<Self, UsvfsError> {
        let lib = unsafe { Library::new(dll_path) }
            .map_err(|e| UsvfsError::LoadLibrary(dll_path.to_path_buf(), e))?;

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

        Ok(Self {
            lib,
            instance_name: instance_name.to_string(),
        })
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
    /// view. Waits up to `timeout` and returns the exit code.
    ///
    /// Deliberately doesn't wire up stdout/stderr pipes — callers that want
    /// output should have the command redirect to a file *outside* any
    /// linked directory (e.g. `cmd /c dir ... > out.txt`); that file write
    /// isn't virtualized, it's a normal disk write, so it's readable back
    /// from this process once `run_hooked` returns.
    pub fn run_hooked(&self, command_line: &str, timeout: Duration) -> Result<u32, UsvfsError> {
        let mut cmdline_w = to_wide_mut(command_line);
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
                CREATE_NO_WINDOW,
                std::ptr::null(),
                std::ptr::null(),
                &mut startup_info,
                &mut process_info,
            )
        };

        if ok == 0 {
            return Err(UsvfsError::CreateProcessHookedFailed(
                command_line.to_string(),
            ));
        }

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

    #[must_use]
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }
}

/// Virtually link every *enabled* entry in `entries` onto `data_dir`, in the
/// order given. `loadout::entries` already returns rows in `position`
/// ascending order, which — now confirmed for real (see
/// `two_mods_overlay_onto_one_destination_last_linked_wins`) — means the
/// highest-position enabled mod wins any file conflict, consistent with
/// `modlab_core::conflicts`. Returns how many mods were linked. Doesn't
/// touch a single real file: `data_dir` itself is never written to.
pub fn deploy_enabled_mods(
    vfs: &Vfs,
    entries: &[modlab_core::loadout::LoadoutEntryRow],
    data_dir: &Path,
) -> Result<usize, UsvfsError> {
    let mut linked = 0;
    for entry in entries.iter().filter(|e| e.enabled) {
        vfs.link_directory_recursive(&entry.source_path, data_dir)?;
        linked += 1;
    }
    Ok(linked)
}

impl Drop for Vfs {
    fn drop(&mut self) {
        // Best-effort: if the symbol lookup itself fails there's nothing
        // more we can do from a Drop impl, and nothing on real disk was
        // ever touched regardless.
        unsafe {
            if let Ok(disconnect) = self.lib.get::<FnDisconnectVfs>(b"DisconnectVFS\0") {
                disconnect();
            }
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
}
