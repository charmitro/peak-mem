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
                "Cannot access process {pid} memory info"
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
        // Use libproc to get process list - the modern macOS approach
        // This is more reliable than parsing sysctl's kinfo_proc structure
        // which has undocumented layout changes between macOS versions
        use std::ptr;

        // External functions from libproc
        extern "C" {
            fn proc_listpids(
                type_: u32,
                typeinfo: u32,
                buffer: *mut libc::c_void,
                buffersize: libc::c_int,
            ) -> libc::c_int;

            fn proc_pidinfo(
                pid: libc::c_int,
                flavor: libc::c_int,
                arg: u64,
                buffer: *mut libc::c_void,
                buffersize: libc::c_int,
            ) -> libc::c_int;
        }

        const PROC_ALL_PIDS: u32 = 1;
        const PROC_PIDTBSDINFO: libc::c_int = 3;

        #[repr(C)]
        struct proc_bsdinfo {
            pbi_flags: u32,
            pbi_status: u32,
            pbi_xstatus: u32,
            pbi_pid: u32,
            pbi_ppid: u32,
            pbi_uid: libc::uid_t,
            pbi_gid: libc::gid_t,
            pbi_ruid: libc::uid_t,
            pbi_rgid: libc::gid_t,
            pbi_svuid: libc::uid_t,
            pbi_svgid: libc::gid_t,
            rfu_1: u32,
            pbi_comm: [libc::c_char; 16],
            pbi_name: [libc::c_char; 32],
            pbi_nfiles: u32,
            pbi_pgid: u32,
            pbi_pjobc: u32,
            e_tdev: u32,
            e_tpgid: u32,
            pbi_nice: libc::c_int,
            pbi_start_tvsec: u64,
            pbi_start_tvusec: u64,
        }

        // Get the size needed for all PIDs
        let buffer_size = unsafe { proc_listpids(PROC_ALL_PIDS, 0, ptr::null_mut(), 0) };

        if buffer_size <= 0 {
            return Err(PeakMemError::Monitor(
                "Failed to get process list size".to_string(),
            ));
        }

        // Allocate buffer for PIDs
        let pid_count = (buffer_size as usize) / mem::size_of::<libc::pid_t>();
        let mut pids = vec![0 as libc::pid_t; pid_count];

        // Get all PIDs
        let bytes_returned = unsafe {
            proc_listpids(
                PROC_ALL_PIDS,
                0,
                pids.as_mut_ptr() as *mut libc::c_void,
                buffer_size,
            )
        };

        if bytes_returned <= 0 {
            return Err(PeakMemError::Monitor(
                "Failed to get process list".to_string(),
            ));
        }

        let actual_pid_count = (bytes_returned as usize) / mem::size_of::<libc::pid_t>();
        let mut children = Vec::new();

        // Check each PID to see if it's a child of our target
        for &check_pid in pids.iter().take(actual_pid_count) {
            if check_pid == 0 {
                continue;
            }

            let mut proc_info: proc_bsdinfo = unsafe { mem::zeroed() };
            let ret = unsafe {
                proc_pidinfo(
                    check_pid,
                    PROC_PIDTBSDINFO,
                    0,
                    &mut proc_info as *mut _ as *mut libc::c_void,
                    mem::size_of::<proc_bsdinfo>() as libc::c_int,
                )
            };

            if ret == mem::size_of::<proc_bsdinfo>() as libc::c_int && proc_info.pbi_ppid == pid {
                children.push(check_pid as u32);
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
        return Ok(format!("pid:{pid}"));
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
        .unwrap_or(&format!("pid:{pid}"))
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
