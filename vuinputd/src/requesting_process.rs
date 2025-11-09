// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use log::debug;
use nix::{
    sched::{setns, CloneFlags},
    unistd::{fork, ForkResult},
};
use std::{
    fs::{self, File}, io::Read, os::fd::AsFd, path::{self, Path}, process, thread, time::Duration
};

use std::io::{self, BufRead};
use std::path::PathBuf;


#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Pid {
    SelfPid,
    Pid(i32),
}

impl Pid {
    pub fn path(&self) -> String {
        match self {
            Pid::SelfPid => "/proc/self".to_string(),
            Pid::Pid(pid_no) => format!("/proc/{}",pid_no)
        }
    }
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
        Pid::SelfPid =>
            unreachable!()
    }
}

// TODO: Rename to capture all relevant process information
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct RequestingProcess {
    pub nspath: String,
    pub nsroot: String,
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
        writeln!(f, "  pid_for_children:  {:?}", self.namespaces.pid_for_children)?;
        writeln!(f, "  user: {:?}", self.namespaces.user)?;
        writeln!(f, "  mnt:  {:?}", self.namespaces.mnt)?;
        writeln!(f, "  cgroup:  {:?}", self.namespaces.cgroup)?;
        writeln!(f, "  time:  {:?}", self.namespaces.time)?;
        writeln!(f, "  time_for_children:  {:?}", self.namespaces.time_for_children)?;
        Ok(())
    }
}

pub fn get_namespace(pid: Pid) -> Namespaces {
    let pid: String = match pid {
        Pid::Pid(pid) => pid.to_string(),
        Pid::SelfPid => "self".to_string(),
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
    let content =
        match pid {
            Pid::SelfPid => fs::read_to_string(format!("/proc/self/status")).ok()?,
            Pid::Pid(pid) => fs::read_to_string(format!("/proc/{}/status", pid)).ok()?
        };
    let ppid=content
        .lines()
        .find(|line| line.starts_with("PPid:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|ppid| ppid.parse::<i32>().ok());
    match ppid {
        None => None,
        Some(ppid)=> Some(Pid::Pid(ppid))
    }
}



pub fn get_requesting_process(pid: Pid) -> RequestingProcess {

    match pid {
        Pid::Pid(_) =>
        {
            let is_compat = match is_compat_process(pid) {
                Some(false) => {
                    debug!("identified process {} as 64 bit process",pid.path());
                    false
                },
                Some(true) => {
                    debug!("identified process {} as 32 bit process",pid.path());
                    true
                },
                None => {
                    debug!("could not identify bitness of process {}. Assume 64 bit process",pid.path());
                    false
                },
            };


            // go up the parent hierarchy until we find a parent with different namespaces
            let mut ppid = pid;
            let nsinodes = get_namespace(pid);
            loop {
                let candidate_ppid = get_ppid(ppid);
                match candidate_ppid {
                    None => break,
                    Some(candidate_ppid) =>
                    {
                        let ppid_nsinodes = get_namespace(candidate_ppid);
                        if nsinodes.equal_mnt_and_net(&ppid_nsinodes) {
                            ppid=candidate_ppid;
                        } else {
                            break;
                        }
                    }

                }
            }
            debug!("identified process {} as root of process id {}",ppid.path(),pid.path());

            let nspath = format!("{}/ns", pid.path());
            let nsroot = format!("{}/ns", ppid.path());
            RequestingProcess {
                nspath: nspath,
                nsroot: nsroot,
                namespaces: nsinodes,
                is_compat: is_compat
            }
        },
        Pid::SelfPid =>
        {
            unreachable!();
        },
    }
}

/// Runs a function inside the given network and mount namespaces.
/// Returns the child PID so the caller can `waitpid` on it.
pub fn run_in_net_and_mnt_namespace(ns: RequestingProcess, func: Box<dyn Fn()>) -> nix::Result<nix::unistd::Pid> {
    //Note: The child process is created with a single thread—the one that called fork().

    match unsafe { fork()? } {
        ForkResult::Parent { child } => {
            // Parent: return the PID of the child
            Ok(child)
        }
        ForkResult::Child => {
            debug!("Start new process {}",process::id());
            // enter namespace
            let path: &Path = Path::new(ns.nsroot.as_str());
            debug!("Entering namespaces of process {}. We assume this is the root process of the container.",ns.nsroot.clone());
            if !fs::exists(path).unwrap() {
                debug!("the root process of the container whose namespaces we want to enter does not exist anymore!");
                std::process::exit(0);
            }
            let net = File::open(ns.nsroot.clone() + "/net").expect("net not found");
            let mnt = File::open(ns.nsroot.clone() + "/mnt").expect("mnt not found");
            setns(net.as_fd(), CloneFlags::CLONE_NEWNET).expect("couldn't enter net");
            setns(mnt.as_fd(), CloneFlags::CLONE_NEWNS).expect("couldn't enter mnt");
            
            // execute your function
            func();
            std::process::exit(0);
        }
    }
}
