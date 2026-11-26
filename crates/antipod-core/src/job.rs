use crate::CoreError;

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::process::Command;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    use crate::CoreError;

    pub struct JobObject {
        handle: HANDLE,
    }

    // SAFETY: Win32 Job Object handle является потокобезопасным объектом ядра ОС.
    unsafe impl Send for JobObject {}

    impl JobObject {
        pub fn new_kill_on_close() -> Result<Self, CoreError> {
            unsafe {
                let handle = CreateJobObjectW(None, PCWSTR::null())
                    .map_err(|e| CoreError::Process(format!("CreateJobObjectW: {e}")))?;

                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                if let Err(e) = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<c_void>(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) {
                    let _ = CloseHandle(handle);
                    return Err(CoreError::Process(format!("SetInformationJobObject: {e}")));
                }

                Ok(Self { handle })
            }
        }

        pub fn assign_pid(&self, pid: u32) -> Result<(), CoreError> {
            unsafe {
                let proc_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid)
                    .map_err(|e| CoreError::Process(format!("OpenProcess({pid}): {e}")))?;
                let res = AssignProcessToJobObject(self.handle, proc_handle);
                let _ = CloseHandle(proc_handle);
                res.map_err(|e| CoreError::Process(format!("AssignProcessToJobObject({pid}): {e}")))
            }
        }

        pub fn terminate(&self) -> Result<(), CoreError> {
            if !self.handle.is_invalid() {
                unsafe {
                    TerminateJobObject(self.handle, 1)
                        .map_err(|e| CoreError::Process(format!("TerminateJobObject: {e}")))?;
                }
            }
            Ok(())
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            if !self.handle.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self.handle);
                }
            }
        }
    }

    pub fn kill_process_tree(pid: u32) -> Result<(), CoreError> {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|e| CoreError::Process(format!("Не удалось запустить taskkill: {e}")))?;
        if !status.success() {
            return Err(CoreError::Process(format!(
                "Процесс PID {pid} уже завершён или недоступен"
            )));
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::CoreError;

    pub struct JobObject;

    impl JobObject {
        pub fn new_kill_on_close() -> Result<Self, CoreError> {
            Ok(Self)
        }

        pub fn assign_pid(&self, _pid: u32) -> Result<(), CoreError> {
            Ok(())
        }

        pub fn terminate(&self) -> Result<(), CoreError> {
            Ok(())
        }
    }

    pub fn kill_process_tree(_pid: u32) -> Result<(), CoreError> {
        Err(CoreError::Process(
            "Поддерживается только на Windows".to_string(),
        ))
    }
}

pub use imp::{kill_process_tree, JobObject};

pub fn make_managed_job(pid: u32) -> Result<JobObject, CoreError> {
    let job = JobObject::new_kill_on_close()?;
    job.assign_pid(pid)?;
    Ok(job)
}
