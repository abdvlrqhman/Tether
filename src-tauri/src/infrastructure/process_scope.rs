//! Owned process lifetime. Windows jobs contain descendants; Unix shells own process groups.
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE},
    },
};
pub struct ProcessScope {
    #[cfg(windows)]
    job: HANDLE,
    #[cfg(unix)]
    pid: u32,
}
// Windows kernel handles are thread safe; this handle is closed exactly once in Drop.
#[cfg(windows)]
unsafe impl Send for ProcessScope {}
#[cfg(windows)]
unsafe impl Sync for ProcessScope {}
impl ProcessScope {
    pub fn new(pid: u32) -> Result<Self, String> {
        #[cfg(windows)]
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                CloseHandle(job);
                return Err(std::io::Error::last_os_error().to_string());
            }
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                CloseHandle(job);
                return Err(std::io::Error::last_os_error().to_string());
            }
            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                CloseHandle(job);
                return Err(format!(
                    "Cannot contain remote process: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(Self { job })
        }
        #[cfg(unix)]
        {
            Ok(Self { pid })
        }
    }
    pub fn terminate(&self) {
        #[cfg(windows)]
        unsafe {
            TerminateJobObject(self.job, 1);
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.pid as i32), libc::SIGKILL);
        }
    }
}
impl Drop for ProcessScope {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            CloseHandle(self.job);
        }
        #[cfg(unix)]
        self.terminate();
    }
}
