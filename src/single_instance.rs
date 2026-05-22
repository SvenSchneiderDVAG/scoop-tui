#[cfg(target_os = "windows")]
use std::io;

#[cfg(target_os = "windows")]
const INSTANCE_MUTEX_NAME: &str = "Local\\scoop-tui-single-instance";

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub struct SingleInstanceGuard {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
impl SingleInstanceGuard {
    pub fn acquire() -> io::Result<Option<Self>> {
        Self::acquire_named(INSTANCE_MUTEX_NAME)
    }

    fn acquire_named(name: &str) -> io::Result<Option<Self>> {
        use windows_sys::Win32::Foundation::{
            CloseHandle, GetLastError, SetLastError, ERROR_ALREADY_EXISTS,
        };
        use windows_sys::Win32::System::Threading::CreateMutexW;

        let name = wide_null(name);
        unsafe {
            SetLastError(0);
        }

        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }

        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                CloseHandle(handle);
            }
            return Ok(None);
        }

        Ok(Some(Self { handle }))
    }
}

#[cfg(target_os = "windows")]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug)]
pub struct SingleInstanceGuard;

#[cfg(not(target_os = "windows"))]
impl SingleInstanceGuard {
    pub fn acquire() -> std::io::Result<Option<Self>> {
        Ok(Some(Self))
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    #[test]
    fn acquiring_same_named_mutex_twice_reports_existing_instance() {
        let name = format!("Local\\scoop-tui-test-{}", std::process::id());

        let first = SingleInstanceGuard::acquire_named(&name).expect("first mutex");
        let second = SingleInstanceGuard::acquire_named(&name).expect("second mutex");

        assert!(first.is_some());
        assert!(second.is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_returns_guard_for_current_platform() {
        assert!(SingleInstanceGuard::acquire()
            .expect("single instance")
            .is_some());
    }
}
