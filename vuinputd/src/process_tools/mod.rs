// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use async_io::Async;
use base64::prelude::BASE64_STANDARD;
use base64::Engine as _;
use log::debug;
use std::{
    fs::{self, File},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{fs::MetadataExt, process::CommandExt},
    },
    path::Path,
    process::Command,
    sync::OnceLock,
};

use anyhow::anyhow;
use std::io;

use crate::{
    actions::action::Action,
    global_config::{get_device_owner, DeviceOwner},
};

pub mod ns_fscreds;

pub static SELF_NAMESPACES: OnceLock<Namespaces> = OnceLock::new();

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Pid {
    Pid(u32),
}

impl Pid {
    pub fn path(&self) -> String {
        match self {
            Pid::Pid(pid_no) => format!("/proc/{}", pid_no),
        }
    }
    pub fn to_string_rep(&self) -> String {
        let Pid::Pid(val) = self;
        val.to_string()
    }
}
enum PidOrSelf {
    Pid(u32),
    SelfPid,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Namespaces {
    pub net: Option<u64>,
    pub uts: Option<u64>,
    pub ipc: Option<u64>,
    pub pid: Option<u64>,
    pub pid_for_children: Option<u64>,
    pub user: Option<u64>,
    pub mnt: Option<u64>,
    pub cgroup: Option<u64>,
    pub time: Option<u64>,
    pub time_for_children: Option<u64>,
}

/// Returns true if the process with `pid` is a 32-bit (compat) process. None, if unsure.
pub fn is_compat_process(pid: Pid) -> Option<bool> {
    match pid {
        Pid::Pid(pid) => {
            const EI_CLASS: usize = 4;
            const ELFCLASS32: u8 = 1;
            const ELFCLASS64: u8 = 2;

            let exe_path = format!("/proc/{}/exe", pid);
            let mut buf = [0u8; 5];

            match File::open(&exe_path).and_then(|mut f| f.read_exact(&mut buf)) {
                Ok(()) => {
                    // ELF magic check
                    if &buf[0..4] != b"\x7FELF" {
                        return None;
                    }
                    match buf[EI_CLASS] {
                        ELFCLASS32 => Some(true),
                        ELFCLASS64 => Some(false),
                        _ => None,
                    }
                }
                Err(_) => None,
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RequestingProcess {
    pub pid_requestor: Pid,
    pub pid_requestor_root: Pid,
    pub namespaces: Namespaces,
    pub is_compat: bool,
}

impl Namespaces {
    pub fn equal_mnt_and_net(&self, other: &Namespaces) -> bool {
        self.mnt == other.mnt && self.net == other.net
    }
}

impl RequestingProcess {
    pub fn equal_mnt_and_net(&self, other: &RequestingProcess) -> bool {
        self.namespaces.equal_mnt_and_net(&other.namespaces)
    }

    pub fn equal_mnt_and_net_ns(&self, other: &Namespaces) -> bool {
        self.namespaces.equal_mnt_and_net(&other)
    }
}

impl std::fmt::Display for RequestingProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Namespaces:")?;
        writeln!(f, "  net:  {:?}", self.namespaces.net)?;
        writeln!(f, "  uts:  {:?}", self.namespaces.uts)?;
        writeln!(f, "  ipc:  {:?}", self.namespaces.ipc)?;
        writeln!(f, "  pid:  {:?}", self.namespaces.pid)?;
        writeln!(
            f,
            "  pid_for_children:  {:?}",
            self.namespaces.pid_for_children
        )?;
        writeln!(f, "  user: {:?}", self.namespaces.user)?;
        writeln!(f, "  mnt:  {:?}", self.namespaces.mnt)?;
        writeln!(f, "  cgroup:  {:?}", self.namespaces.cgroup)?;
        writeln!(f, "  time:  {:?}", self.namespaces.time)?;
        writeln!(
            f,
            "  time_for_children:  {:?}",
            self.namespaces.time_for_children
        )?;
        Ok(())
    }
}

pub fn get_self_namespace() -> Namespaces {
    get_namespace_of_pid_or_self(PidOrSelf::SelfPid)
}

pub fn get_namespace(pid: Pid) -> Namespaces {
    let Pid::Pid(pid) = pid;
    get_namespace_of_pid_or_self(PidOrSelf::Pid(pid))
}

fn get_namespace_of_pid_or_self(pid_or_self: PidOrSelf) -> Namespaces {
    let pid: String = match pid_or_self {
        PidOrSelf::Pid(pid) => pid.to_string(),
        PidOrSelf::SelfPid => "self".to_string(),
    };
    let nspath = format!("/proc/{}/ns", pid);

    let mut ns = Namespaces {
        net: None,
        uts: None,
        ipc: None,
        pid: None,
        pid_for_children: None,
        user: None,
        mnt: None,
        cgroup: None,
        time: None,
        time_for_children: None,
    };

    for entry in fs::read_dir(&nspath).expect("proc not found") {
        let entry = entry.expect("`msg`");
        let link = fs::read_link(entry.path()).expect("problem parsing inode");
        let link_str = link.to_string_lossy();
        if let (Some(start), Some(end)) = (link_str.find('['), link_str.find(']')) {
            if let Ok(inode) = link_str[start + 1..end].parse::<u64>() {
                match entry.file_name().into_string().unwrap_or_default().as_str() {
                    "net" => ns.net = Some(inode),
                    "uts" => ns.uts = Some(inode),
                    "ipc" => ns.ipc = Some(inode),
                    "pid" => ns.pid = Some(inode),
                    "pid_for_children" => ns.pid_for_children = Some(inode),
                    "user" => ns.user = Some(inode),
                    "mnt" => ns.mnt = Some(inode),
                    "cgroup" => ns.cgroup = Some(inode),
                    "time" => ns.time = Some(inode),
                    "time_for_children" => ns.time_for_children = Some(inode),
                    _ => (),
                }
            }
        }
    }
    ns
}

fn get_ppid(pid: Pid) -> Option<Pid> {
    let content = match pid {
        Pid::Pid(pid) => fs::read_to_string(format!("/proc/{}/status", pid)).ok()?,
    };
    let ppid = content
        .lines()
        .find(|line| line.starts_with("PPid:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|ppid| ppid.parse::<u32>().ok());
    match ppid {
        None => None,
        Some(ppid) => Some(Pid::Pid(ppid)),
    }
}

pub fn get_requesting_process(pid: Pid) -> RequestingProcess {
    match pid {
        Pid::Pid(_) => {
            let is_compat = match is_compat_process(pid) {
                Some(false) => {
                    debug!("identified process {} as 64 bit process", pid.path());
                    false
                }
                Some(true) => {
                    debug!("identified process {} as 32 bit process", pid.path());
                    true
                }
                None => {
                    debug!(
                        "could not identify bitness of process {}. Assume 64 bit process",
                        pid.path()
                    );
                    false
                }
            };

            // go up the parent hierarchy until we find a parent with different namespaces
            let mut ppid = pid;
            let nsinodes = get_namespace(pid);
            loop {
                let candidate_ppid = get_ppid(ppid);
                match candidate_ppid {
                    None => break,
                    Some(candidate_ppid) => {
                        let ppid_nsinodes = get_namespace(candidate_ppid);
                        if nsinodes.equal_mnt_and_net(&ppid_nsinodes) {
                            ppid = candidate_ppid;
                        } else {
                            break;
                        }
                    }
                }
            }
            debug!(
                "identified process {} as root of process id {}",
                ppid.path(),
                pid.path()
            );

            RequestingProcess {
                pid_requestor: pid,
                pid_requestor_root: ppid,
                namespaces: nsinodes,
                is_compat: is_compat,
            }
        }
    }
}

fn print_debug_string(action: &str, ns: &RequestingProcess) {
    let action_base64 = BASE64_STANDARD.encode(action);
    let mut debugstring = String::new();
    debugstring.push_str("In case you need to debug the system calls, call `strace vuinputd");
    debugstring.push_str(" --target-pid ");
    debugstring.push_str(&ns.pid_requestor_root.to_string_rep());
    debugstring.push_str(" --action-base64 ");
    debugstring.push_str(action_base64.as_str());
    debugstring.push_str(" --device-owner ");
    debugstring.push_str(get_device_owner().to_string_rep().as_str());
    debugstring.push_str("`");
    debug!("{}", debugstring);
}

/// Runs a function inside the given network and mount namespaces.
/// Returns the child PID so the caller can `waitpid` on it.
pub fn start_action(
    action: Action,
    ns: &RequestingProcess,
    enter_user_ns: bool,
) -> anyhow::Result<u32> {
    let action_json = serde_json::to_string(&action).unwrap();
    print_debug_string(&action_json, &ns);

    let device_owner = get_device_owner().to_string_rep();

    let child = unsafe {
        let mut cmd = Command::new("/proc/self/exe");
        cmd.args([
            "--action",
            action_json.as_str(),
            "--target-pid",
            ns.pid_requestor_root.to_string_rep().as_str(),
            "--device-owner",
            device_owner.as_str(),
        ]);
        if enter_user_ns {
            cmd.arg("--enter-user-namespace");
        }
        cmd.pre_exec(|| {
            // Last resort, if the parent just is killed.
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        })
        .spawn()
        .expect("failed to start vuinputd")
    };

    Result::Ok(child.id())
}

pub fn run_in_net_and_mnt_namespace(
    target_pid: &str,
    device_owner: &DeviceOwner,
    enter_user_ns: bool,
) -> anyhow::Result<()> {
    debug!(
        "Entering namespaces of process {}. We assume this is the root process of the container.",
        target_pid
    );

    let fs_uid_gid = if *device_owner == DeviceOwner::ContainerDevFolder {
        let pid: u32 = target_pid.trim().parse()?;
        let pid = Pid::Pid(pid);
        let fs_uid = ns_fscreds::get_uid_in_container(pid, 0)?;
        let fs_gid = ns_fscreds::get_gid_in_container(pid, 0)?;
        Some((fs_uid, fs_gid))
    } else {
        None
    };

    let nspath = format!("/proc/{}/ns", target_pid);
    let path: &Path = Path::new(&nspath);
    if !fs::exists(path).unwrap() {
        return Err(anyhow!("the root process of the container whose namespaces we want to enter does not exist anymore"));
    }
    let user = File::open(nspath.to_string() + "/user")?;
    let net = File::open(nspath.to_string() + "/net")?;
    let mnt = File::open(nspath.to_string() + "/mnt")?;

    unsafe {
        // enter namespaces
        if enter_user_ns {
            libc::setns(user.as_raw_fd(), libc::CLONE_NEWUSER);
            libc::setresgid(0, 0, 0);
            libc::setresuid(0, 0, 0);
        }
        libc::setns(net.as_raw_fd(), libc::CLONE_NEWNET);
        libc::setns(mnt.as_raw_fd(), libc::CLONE_NEWNS);
    };

    if let Some((fs_uid, fs_gid)) = fs_uid_gid {
        ns_fscreds::acquire_uid_and_gid(fs_uid, fs_gid)?;
    }

    anyhow::Ok(())
}

pub async fn await_process(pid: Pid) -> io::Result<i32> {
    match pid {
        Pid::Pid(pid) => {
            unsafe {
                // Use pidfd_open() (libc) to get a real FD
                let pidfd = libc::syscall(libc::SYS_pidfd_open, pid, 0);
                if pidfd == -1 {
                    return Err(io::Error::last_os_error());
                }
                let owned_fd = OwnedFd::from_raw_fd(pidfd as RawFd);

                // Wait asynchronously on the pidfd
                let async_adapter = Async::new(owned_fd)?;
                async_adapter.readable().await?;

                // Retrieve the exit code using waitid()
                let mut si: libc::siginfo_t = std::mem::zeroed();
                let r = libc::waitid(libc::P_PID, pid as u32, &mut si, libc::WEXITED);
                if r != 0 {
                    return Err(io::Error::last_os_error());
                }

                Ok(si.si_status())
            }
        }
    }
}

pub fn check_permissions() -> Result<(), std::io::Error> {
    let path = Path::new("/proc/self/status");
    debug!("Capabilities of vuinputd process:");
    fs::read_to_string(path).and_then(|status_file| {
        status_file
            .lines()
            .filter(|line| line.starts_with("Cap"))
            .for_each(move |x| debug!("{}", x));
        Ok(())
    })
}
