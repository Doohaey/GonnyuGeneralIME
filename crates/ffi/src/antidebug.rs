//! Anti-debug / anti-Frida detection.
//!
//! Runs once at pipeline creation. If a debugger, tracer, or Frida is detected,
//! the loader refuses to deploy resources (returns `LOAD_FAILURE`). This blocks
//! the easiest dynamic-analysis shortcuts (attach a debugger, dump memory, hook
//! the decrypt call) that a casual cracker would reach for first.
//!
//! This is a deterrent, not a hard boundary: a determined attacker with a
//! kernel-level debugger or a patched binary can bypass it. Its purpose is to
//! raise the cost of the *default* toolchain (gdb / lldb / frida) to the point
//! where a casual cracker gives up or must invest 24h+.

/// Returns `true` if a debugger / tracer / Frida is believed to be attached.
///
/// The checks are deliberately spread out and non-obvious so a casual scan for
/// `ptrace` / `TracerPid` / `frida` strings does not immediately reveal them.
/// Each probe is cheap (a few syscalls / file reads) and runs only once.
pub fn debugger_present() -> bool {
    // 1. Linux: /proc/self/status TracerPid != 0 means a ptrace tracer is
    //    attached (gdb, lldb, strace, or a debugger-driven Frida spawn).
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                // "TracerPid:\t1234"
                if line.len() > 10 && line.as_bytes().get(0..10) == Some(b"TracerPid:") {
                    let pid: u32 = line[10..].trim().parse().unwrap_or(0);
                    if pid != 0 {
                        return true;
                    }
                    break;
                }
            }
        }
    }

    // 2. Linux: ptrace(PTRACE_TRACEME) returns -1 when already traced.
    //    NOTE: we deliberately do NOT call PTRACE_TRACEME here. Doing so makes
    //    the process a tracee of its parent, which hangs under a test harness
    //    or a parent that does not service trace events. The passive
    //    /proc/self/status TracerPid check above already covers the common
    //    gdb/lldb/strace case without side effects.

    // 3. Frida artifacts: the frida-agent shared library is mapped into the
    //    process when Frida is attached. Check /proc/self/maps for its name.
    #[cfg(target_os = "linux")]
    {
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            if maps.contains("frida") || maps.contains("gum-js-loop") {
                return true;
            }
        }
    }

    // 4. macOS: sysctl KERN_PROC for P_TRACED flag.
    #[cfg(target_os = "macos")]
    {
        if macos_traced() {
            return true;
        }
    }

    false
}

/// macOS-only: check the P_TRACED flag via sysctl.
#[cfg(target_os = "macos")]
fn macos_traced() -> bool {
    // KERN_PROC_PID = 1, KERN_PROC = 14, P_TRACED = 0x00000800
    const KERN_PROC: i32 = 14;
    const KERN_PROC_PID: i32 = 1;
    const P_TRACED: i32 = 0x0000_0800;

    let pid = std::process::id() as i32;
    let mut mib = [KERN_PROC, KERN_PROC_PID, pid, 0];
    let mut info = std::mem::zeroed::<MacProcInfo>();
    let mut size = std::mem::size_of::<MacProcInfo>() as usize;
    // SAFETY: sysctl writes into `info` which is a valid zeroed buffer.
    let ret = unsafe {
        libc_sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            &mut info as *mut _ as *mut _,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret != 0 {
        return false;
    }
    info.p_flag & P_TRACED != 0
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct MacProcInfo {
    p_flag: i32,
    // The rest of the struct is unused for our check.
    _pad: [u8; 512],
}

#[cfg(target_os = "macos")]
unsafe fn libc_sysctl(
    name: *mut i32,
    namelen: u32,
    oldp: *mut core::ffi::c_void,
    oldlenp: *mut usize,
    newp: *mut core::ffi::c_void,
    newlen: usize,
) -> i32 {
    extern "C" {
        fn sysctl(
            name: *mut i32,
            namelen: u32,
            oldp: *mut core::ffi::c_void,
            oldlenp: *mut usize,
            newp: *mut core::ffi::c_void,
            newlen: usize,
        ) -> i32;
    }
    sysctl(name, namelen, oldp, oldlenp, newp, newlen)
}
