//! Making sure JMeter never outlives the app. On Windows a child process keeps running when its parent ends,
//! so each JMeter process is put in a job object that ends all its processes when the job's last handle is
//! closed, which Windows does when the app ends for any reason, a crash included.

use std::process::Child;

/// Put `child` in the app's kill-on-close job. Returns whether that worked (it does nothing and returns
/// `true` on systems where the child is ended by other means).
#[cfg(windows)]
pub fn tie_to_this_process(child: &Child) -> bool {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    // The job lives as long as the app: it is never closed by us, only by Windows when the app ends.
    static JOB: OnceLock<usize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let job: HANDLE = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32);
        if ok == 0 { 0 } else { job as usize }
    });
    if job == 0 {
        return false;
    }
    unsafe { AssignProcessToJobObject(job as HANDLE, child.as_raw_handle() as HANDLE) != 0 }
}

#[cfg(not(windows))]
pub fn tie_to_this_process(_child: &Child) -> bool {
    true
}
