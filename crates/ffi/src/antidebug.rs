pub fn debugger_present() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
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

    #[cfg(target_os = "linux")]
    {
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            if maps.contains("frida") || maps.contains("gum-js-loop") {
                return true;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if macos_traced() {
            return true;
        }
    }

    false
}

#[cfg(target_os = "macos")]
fn macos_traced() -> bool {
    const KERN_PROC: i32 = 14;
    const KERN_PROC_PID: i32 = 1;
    const P_TRACED: i32 = 0x0000_0800;

    let pid = std::process::id() as i32;
    let mut mib = [KERN_PROC, KERN_PROC_PID, pid, 0];
    let mut info = unsafe { std::mem::zeroed::<MacProcInfo>() };
    let mut size = std::mem::size_of::<MacProcInfo>() as usize;
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
