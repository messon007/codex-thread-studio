use std::{
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

// On Linux this names the running inode even after an atomic rebuild/install.
// Resolving current_exe() to a pathname instead yields "... (deleted)" and
// permanently prevents new index/compare workers in an otherwise healthy app.
pub fn self_executable() -> std::io::Result<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Ok(std::path::PathBuf::from("/proc/self/exe"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::current_exe()
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn resident_kib(pid: u32) -> Option<usize> {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {}

#[cfg(target_os = "macos")]
pub(crate) fn resident_kib(pid: u32) -> Option<usize> {
    // libproc returns resident bytes, unlike getrusage's peak RSS for this process.
    unsafe {
        let mut usage: libc::rusage_info_v2 = std::mem::zeroed();
        let result = libc::proc_pid_rusage(
            pid as libc::c_int,
            libc::RUSAGE_INFO_V2,
            (&mut usage as *mut libc::rusage_info_v2).cast(),
        );
        (result == 0).then_some((usage.ri_resident_size / 1024) as usize)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn resident_kib(pid: u32) -> Option<usize> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Counters {
        size: u32,
        faults: u32,
        peak_working_set: usize,
        working_set: usize,
        peak_paged_pool: usize,
        paged_pool: usize,
        peak_nonpaged_pool: usize,
        nonpaged_pool: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn K32GetProcessMemoryInfo(handle: *mut c_void, counters: *mut Counters, size: u32) -> i32;
    }
    unsafe {
        let process = OpenProcess(0x0400 | 0x0010, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut counters: Counters = std::mem::zeroed();
        counters.size = std::mem::size_of::<Counters>() as u32;
        let result = K32GetProcessMemoryInfo(
            process,
            &mut counters,
            std::mem::size_of::<Counters>() as u32,
        );
        CloseHandle(process);
        (result != 0).then_some(counters.working_set / 1024)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) fn resident_kib(_: u32) -> Option<usize> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn supervision_reads_the_current_process_resident_memory() {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        assert!(super::resident_kib(std::process::id()).is_some_and(|rss| rss > 0));
    }
}

pub(crate) fn supervise(
    child: &mut dyn process_wrap::std::ChildWrapper,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    supervise_budget(child, cancelled, Duration::from_secs(60), 1024 * 1024)
}

pub(crate) fn supervise_budget(
    child: &mut dyn process_wrap::std::ChildWrapper,
    cancelled: &AtomicBool,
    timeout: Duration,
    rss_kib: usize,
) -> Result<(), String> {
    let stderr = child.stderr().take();
    let diagnostic = thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stderr) = stderr {
            let _ = stderr.take(4096).read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        let resident = resident_kib(child.id());
        let error = if cancelled.load(Ordering::Relaxed) {
            Some("Document closed")
        } else if started.elapsed() > timeout || resident.is_some_and(|kib| kib > rss_kib) {
            Some("Worker exceeded its time or memory budget")
        } else {
            None
        };
        if let Some(error) = error {
            let _ = child.kill();
            let _ = child.wait();
            break Err(error.to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(Some(status)) => break Err(format!("Indexing failed ({status})")),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.to_string());
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
        }
    };
    let diagnostic = diagnostic.join().unwrap_or_default();
    status.map_err(|error| {
        format!(
            "{error}: {}",
            diagnostic.chars().take(800).collect::<String>()
        )
    })
}
