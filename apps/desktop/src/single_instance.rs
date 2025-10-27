/// Custom single-instance implementation that handles stale locks
///
/// This module provides a more robust single-instance check than the `single-instance` crate.
/// It stores the process ID in the lock file and checks if that process is still running.
/// If the process is dead, it cleans up the stale lock automatically.
use anyhow::{Context, Result};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct SingleInstanceGuard {
    lock_file: PathBuf,
}

impl SingleInstanceGuard {
    /// Try to acquire a single-instance lock
    ///
    /// Returns Ok(guard) if this is the only instance, or an error if another instance is running
    pub fn acquire(app_name: &str) -> Result<Self> {
        let lock_file = get_lock_file_path(app_name)?;

        // Check if lock file exists
        if lock_file.exists() {
            // Read the PID from the lock file
            let mut file = fs::File::open(&lock_file)
                .context("Failed to open lock file")?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .context("Failed to read lock file")?;

            // Parse the PID
            if let Ok(pid) = contents.trim().parse::<u32>() {
                // Check if the process is still running
                if is_process_running(pid) {
                    anyhow::bail!("Another instance is already running (PID: {})", pid);
                } else {
                    // Process is dead, clean up stale lock
                    tracing::warn!("Found stale lock file with dead process (PID: {}), cleaning up", pid);
                    fs::remove_file(&lock_file)
                        .context("Failed to remove stale lock file")?;
                }
            } else {
                // Invalid PID in lock file, remove it
                tracing::warn!("Found lock file with invalid PID, cleaning up");
                fs::remove_file(&lock_file)
                    .context("Failed to remove invalid lock file")?;
            }
        }

        // Create lock file with current PID
        let current_pid = std::process::id();
        let mut file = fs::File::create(&lock_file)
            .context("Failed to create lock file")?;
        file.write_all(current_pid.to_string().as_bytes())
            .context("Failed to write PID to lock file")?;

        tracing::info!("Acquired single-instance lock (PID: {})", current_pid);

        Ok(Self { lock_file })
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // Clean up lock file when guard is dropped
        if let Err(e) = fs::remove_file(&self.lock_file) {
            tracing::warn!("Failed to remove lock file on exit: {}", e);
        } else {
            tracing::info!("Released single-instance lock");
        }
    }
}

/// Get the path to the lock file
fn get_lock_file_path(app_name: &str) -> Result<PathBuf> {
    // Use platform-specific temp directory
    let temp_dir = std::env::temp_dir();
    Ok(temp_dir.join(format!("{}.lock", app_name)))
}

/// Check if a process with the given PID is running
#[cfg(target_os = "linux")]
fn is_process_running(pid: u32) -> bool {
    // On Linux, check if /proc/<pid> exists
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}

#[cfg(target_os = "macos")]
fn is_process_running(pid: u32) -> bool {
    // On macOS, use kill(pid, 0) to check if process exists
    use std::io::Error;

    unsafe {
        let result = libc::kill(pid as i32, 0);
        if result == 0 {
            true
        } else {
            let err = Error::last_os_error();
            // ESRCH means process doesn't exist
            // EPERM means process exists but we don't have permission (still running)
            err.raw_os_error() != Some(libc::ESRCH)
        }
    }
}

#[cfg(target_os = "windows")]
fn is_process_running(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        // Try to open the process
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);

        if let Ok(handle) = handle {
            if handle.0 != 0 {
                let _ = CloseHandle(handle);
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_process_is_running() {
        let current_pid = std::process::id();
        assert!(is_process_running(current_pid));
    }

    #[test]
    fn test_invalid_process_not_running() {
        // PID 1 is init/systemd on Linux, but we're testing with a very high PID
        // that's unlikely to exist
        assert!(!is_process_running(999999));
    }
}
