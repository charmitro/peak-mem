use crate::monitor::MemoryMonitor;
use crate::types::{MemoryUsage, PeakMemError, ProcessMemoryInfo, Result};
use async_trait::async_trait;
use chrono::Utc;
use std::mem;

pub struct MacOSMonitor;

impl MacOSMonitor {
    pub fn new() -> Result<Self> {
        Ok(MacOSMonitor)
    }

    fn get_memory_for_pid(&self, pid: u32) -> Result<(u64, u64)> {
        use libc::{proc_pidinfo, proc_taskinfo, PROC_PIDTASKINFO};

        let mut info: proc_taskinfo = unsafe { mem::zeroed() };
        let size = mem::size_of::<proc_taskinfo>() as i32;

        let ret = unsafe {
            proc_pidinfo(
                pid as i32,
                PROC_PIDTASKINFO,
                0,
                &mut info as *mut _ as *mut _,
                size,
            )
        };

        if ret <= 0 {
            return Err(PeakMemError::PermissionDenied(format!(
                "Cannot access process {} memory info",
                pid
            )));
        }

        Ok((info.pti_resident_size, info.pti_virtual_size))
    }
}

#[async_trait]
impl MemoryMonitor for MacOSMonitor {
    async fn get_memory_usage(&self, pid: u32) -> Result<MemoryUsage> {
        let (rss_bytes, vsz_bytes) = self.get_memory_for_pid(pid)?;

        Ok(MemoryUsage {
            rss_bytes,
            vsz_bytes,
            timestamp: Utc::now(),
        })
    }

    async fn get_process_tree(&self, pid: u32) -> Result<ProcessMemoryInfo> {
        let memory = self.get_memory_usage(pid).await?;
        let name = get_process_name(pid)?;
        let child_pids = self.get_child_pids(pid).await?;

        let mut children = Vec::new();
        for child_pid in child_pids {
            if let Ok(child_info) = Box::pin(self.get_process_tree(child_pid)).await {
                children.push(child_info);
            }
        }

        Ok(ProcessMemoryInfo {
            pid,
            name,
            memory,
            children,
        })
    }

    async fn get_child_pids(&self, pid: u32) -> Result<Vec<u32>> {
        // Use sysctl to get process list
        use libc::{sysctl, CTL_KERN, KERN_PROC, KERN_PROC_ALL};
        use std::ptr;

        #[repr(C)]
        struct KinfoProcDarwin {
            kp_proc: ExternProc,
            kp_eproc: EProc,
        }

        #[repr(C)]
        struct ExternProc {
            p_un: [u8; 16],
            p_vmspace: u64,
            p_sigacts: u64,
            p_flag: i32,
            p_stat: i8,
            p_pid: i32,
            p_oppid: i32,
            p_dupfd: i32,
            // ... other fields we don't need
        }

        #[repr(C)]
        struct EProc {
            e_paddr: *mut u8,
            e_sess: *mut u8,
            e_pcred: PCredDarwin,
            e_ucred: UCredDarwin,
            e_spare: [i32; 3],
            e_tsess: *mut u8,
            e_wmesg: [i8; 8],
            e_xsize: i32,
            e_xrssize: i16,
            e_xccount: i16,
            e_xswrss: i16,
            e_ppid: i32,
            // ... other fields we don't need
        }

        #[repr(C)]
        struct PCredDarwin {
            pc_lock: [i8; 72],
            pc_ucred: *mut u8,
            p_ruid: u32,
            p_svuid: u32,
            p_rgid: u32,
            p_svgid: u32,
            p_refcnt: i32,
        }

        #[repr(C)]
        struct UCredDarwin {
            cr_ref: i32,
            cr_uid: u32,
            cr_ngroups: i16,
            cr_groups: [u32; 16],
        }

        let mut mib = [CTL_KERN, KERN_PROC, KERN_PROC_ALL, 0];
        let mut size = 0;

        // Get required buffer size
        unsafe {
            if sysctl(
                mib.as_mut_ptr(),
                4,
                ptr::null_mut(),
                &mut size,
                ptr::null_mut(),
                0,
            ) != 0
            {
                return Err(PeakMemError::Monitor(
                    "Failed to get process list size".to_string(),
                ));
            }
        }

        // Allocate buffer
        let mut buf = vec![0u8; size];

        // Get process list
        unsafe {
            if sysctl(
                mib.as_mut_ptr(),
                4,
                buf.as_mut_ptr() as *mut _,
                &mut size,
                ptr::null_mut(),
                0,
            ) != 0
            {
                return Err(PeakMemError::Monitor(
                    "Failed to get process list".to_string(),
                ));
            }
        }

        // Parse process list for children
        let mut children = Vec::new();
        let proc_size = mem::size_of::<KinfoProcDarwin>();
        let proc_count = size / proc_size;

        for i in 0..proc_count {
            let proc = unsafe { &*(buf.as_ptr().add(i * proc_size) as *const KinfoProcDarwin) };

            let ppid = proc.kp_eproc.e_ppid as u32;
            let child_pid = proc.kp_proc.p_pid as u32;

            if ppid == pid && child_pid != 0 {
                children.push(child_pid);
            }
        }

        Ok(children)
    }
}

fn get_process_name(pid: u32) -> Result<String> {
    use libc::{proc_pidpath, PROC_PIDPATHINFO_MAXSIZE};
    use std::ffi::CStr;

    let mut path_buf = vec![0u8; PROC_PIDPATHINFO_MAXSIZE as usize];

    let ret = unsafe {
        proc_pidpath(
            pid as i32,
            path_buf.as_mut_ptr() as *mut _,
            path_buf.len() as u32,
        )
    };

    if ret <= 0 {
        return Ok(format!("pid:{}", pid));
    }

    // Extract just the filename from the path
    let path = unsafe {
        CStr::from_ptr(path_buf.as_ptr() as *const _)
            .to_string_lossy()
            .into_owned()
    };

    Ok(path
        .split('/')
        .next_back()
        .unwrap_or(&format!("pid:{}", pid))
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_memory_usage_self() {
        let monitor = MacOSMonitor::new().unwrap();
        let pid = std::process::id();

        let usage = monitor.get_memory_usage(pid).await;
        assert!(usage.is_ok());

        let usage = usage.unwrap();
        assert!(usage.rss_bytes > 0);
        assert!(usage.vsz_bytes >= usage.rss_bytes);
    }
}
