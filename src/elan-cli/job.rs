// FIXME: stolen from cargo. Should be extracted into a common crate.

//! Job management (mostly for windows)
//!
//! Most of the time when you're running lake you expect Ctrl-C to actually
//! terminate the entire tree of processes in play, not just the one at the top
//! (cago). This currently works "by default" on Unix platforms because Ctrl-C
//! actually sends a signal to the *process group* rather than the parent
//! process, so everything will get torn down. On Windows, however, this does
//! not happen and Ctrl-C just kills lake.
//!
//! To achieve the same semantics on Windows we use Job Objects to ensure that
//! all processes die at the same time. Job objects have a mode of operation
//! where when all handles to the object are closed it causes all child
//! processes associated with the object to be terminated immediately.
//! Conveniently whenever a process in the job object spawns a new process the
//! child will be associated with the job object as well. This means if we add
//! ourselves to the job object we create then everything will get torn down!

pub use self::imp::Setup;

pub fn setup() -> Option<Setup> {
    unsafe { imp::setup() }
}

#[cfg(unix)]
mod imp {
    pub type Setup = ();

    pub unsafe fn setup() -> Option<()> {
        Some(())
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::io;
    use std::mem;
    use std::os::windows::prelude::*;
    use winapi::shared::*;
    use winapi::um::*;

    pub struct Setup {
        job: Handle,
    }

    pub struct Handle {
        inner: ntdef::HANDLE,
    }

    fn last_err() -> io::Error {
        io::Error::last_os_error()
    }

    pub unsafe fn setup() -> Option<Setup> {
        // Creates a new job object for us to use and then adds ourselves to it.
        // Note that all errors are basically ignored in this function,
        // intentionally. Job objects are "relatively new" in Windows,
        // particularly the ability to support nested job objects. Older
        // Windows installs don't support this ability. We probably don't want
        // to force Lake to abort in this situation or force others to *not*
        // use job objects, so we instead just ignore errors and assume that
        // we're otherwise part of someone else's job object in this case.

        let job = jobapi2::CreateJobObjectW(0 as *mut _, 0 as *const _);
        if job.is_null() {
            return None;
        }
        let job = Handle { inner: job };

        // Indicate that when all handles to the job object are gone that all
        // process in the object should be killed. Note that this includes our
        // entire process tree by default because we've added ourselves and and
        // our children will reside in the job once we spawn a process.
        let mut info: winnt::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
        info = mem::zeroed();
        info.BasicLimitInformation.LimitFlags = winnt::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let r = jobapi2::SetInformationJobObject(
            job.inner,
            winnt::JobObjectExtendedLimitInformation,
            &mut info as *mut _ as minwindef::LPVOID,
            mem::size_of_val(&info) as minwindef::DWORD,
        );
        if r == 0 {
            return None;
        }

        // Assign our process to this job object, meaning that our children will
        // now live or die based on our existence.
        let me = processthreadsapi::GetCurrentProcess();
        let r = jobapi2::AssignProcessToJobObject(job.inner, me);
        if r == 0 {
            return None;
        }

        Some(Setup { job: job })
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            // This is a litte subtle. By default if we are terminated then all
            // processes in our job object are terminated as well, but we
            // intentionally want to whitelist some processes to outlive our job
            // object (see below).
            //
            // To allow for this, we manually kill processes instead of letting
            // the job object kill them for us. We do this in a loop to handle
            // processes spawning other processes.
            //
            // Finally once this is all done we know that the only remaining
            // ones are ourselves and the whitelisted processes. The destructor
            // here then configures our job object to *not* kill everything on
            // close, then closes the job object.
            unsafe {
                while self.kill_remaining() {
                    info!("killed some, going for more");
                }

                let mut info: winnt::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
                info = mem::zeroed();
                let r = jobapi2::SetInformationJobObject(
                    self.job.inner,
                    winnt::JobObjectExtendedLimitInformation,
                    &mut info as *mut _ as minwindef::LPVOID,
                    mem::size_of_val(&info) as minwindef::DWORD,
                );
                if r == 0 {
                    info!("failed to configure job object to defaults: {}", last_err());
                }
            }
        }
    }

    impl Setup {
        unsafe fn kill_remaining(&mut self) -> bool {
            #[repr(C)]
            struct Jobs {
                header: winnt::JOBOBJECT_BASIC_PROCESS_ID_LIST,
                list: [basetsd::ULONG_PTR; 1024],
            }

            let mut jobs: Jobs = mem::zeroed();
            let r = jobapi2::QueryInformationJobObject(
                self.job.inner,
                winnt::JobObjectBasicProcessIdList,
                &mut jobs as *mut _ as minwindef::LPVOID,
                mem::size_of_val(&jobs) as minwindef::DWORD,
                0 as *mut _,
            );
            if r == 0 {
                info!("failed to query job object: {}", last_err());
                return false;
            }

            let mut killed = false;
            let list = &jobs.list[..jobs.header.NumberOfProcessIdsInList as usize];
            assert!(list.len() > 0);

            let list = list
                .iter()
                .filter(|&&id| {
                    // let's not kill ourselves
                    id as minwindef::DWORD != processthreadsapi::GetCurrentProcessId()
                })
                .filter_map(|&id| {
                    // Open the process with the necessary rights, and if this
                    // fails then we probably raced with the process exiting so we
                    // ignore the problem.
                    let flags = winnt::PROCESS_QUERY_INFORMATION
                        | winnt::PROCESS_TERMINATE
                        | winnt::SYNCHRONIZE;
                    let p = processthreadsapi::OpenProcess(
                        flags,
                        minwindef::FALSE,
                        id as minwindef::DWORD,
                    );
                    if p.is_null() {
                        None
                    } else {
                        Some(Handle { inner: p })
                    }
                })
                .filter(|p| {
                    // Test if this process was actually in the job object or not.
                    // If it's not then we likely raced with something else
                    // recycling this PID, so we just skip this step.
                    let mut res = 0;
                    let r = jobapi::IsProcessInJob(p.inner, self.job.inner, &mut res);
                    if r == 0 {
                        info!("failed to test is process in job: {}", last_err());
                        return false;
                    }
                    res == minwindef::TRUE
                });

            for p in list {
                // Load the file which this process was spawned from. We then
                // later use this for identification purposes.
                let mut buf = [0; 1024];
                let r = psapi::GetProcessImageFileNameW(
                    p.inner,
                    buf.as_mut_ptr(),
                    buf.len() as minwindef::DWORD,
                );
                if r == 0 {
                    info!("failed to get image name: {}", last_err());
                    continue;
                }
                let s = OsString::from_wide(&buf[..r as usize]);
                info!("found remaining: {:?}", s);

                // And here's where we find the whole purpose for this
                // function!  Currently, our only whitelisted process is
                // `mspdbsrv.exe`, and more details about that can be found
                // here:
                //
                //      https://github.com/rust-lang/rust/issues/33145
                //
                // The gist of it is that all builds on one machine use the
                // same `mspdbsrv.exe` instance. If we were to kill this
                // instance then we could erroneously cause other builds to
                // fail.
                if let Some(s) = s.to_str() {
                    if s.contains("mspdbsrv") {
                        info!("\toops, this is mspdbsrv");
                        continue;
                    }
                }

                // Ok, this isn't mspdbsrv, let's kill the process. After we
                // kill it we wait on it to ensure that the next time around in
                // this function we're not going to see it again.
                let r = processthreadsapi::TerminateProcess(p.inner, 1);
                if r == 0 {
                    info!("\tfailed to kill subprocess: {}", last_err());
                    info!("\tassuming subprocess is dead...");
                } else {
                    info!("\tterminated subprocess");
                }
                let r = synchapi::WaitForSingleObject(p.inner, winbase::INFINITE);
                if r != 0 {
                    info!("failed to wait for process to die: {}", last_err());
                    return false;
                }
                killed = true;
            }

            return killed;
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                handleapi::CloseHandle(self.inner);
            }
        }
    }
}
