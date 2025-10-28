// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use log::debug;
use nix::{
    sched::{setns, CloneFlags},
    unistd::{fork, ForkResult},
};
use std::{
    fs::{self, File},
    os::fd::AsFd, path::{self, Path},
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
struct NamespaceInodes {
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

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Namespaces {
    pub nspath: String,
    pub nsroot: String,
    nsinodes: NamespaceInodes,
}

impl NamespaceInodes {
    pub fn equal_mnt_and_net(&self, other: &NamespaceInodes) -> bool {
        self.mnt == other.mnt && self.net == other.net
    }
}

impl Namespaces {
    pub fn equal_mnt_and_net(&self, other: &Namespaces) -> bool {
        self.nsinodes.equal_mnt_and_net(&other.nsinodes)
    }
}

impl std::fmt::Display for Namespaces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Namespaces:")?;
        writeln!(f, "  net:  {:?}", self.nsinodes.net)?;
        writeln!(f, "  uts:  {:?}", self.nsinodes.uts)?;
        writeln!(f, "  ipc:  {:?}", self.nsinodes.ipc)?;
        writeln!(f, "  pid:  {:?}", self.nsinodes.pid)?;
        writeln!(f, "  pid_for_children:  {:?}", self.nsinodes.pid_for_children)?;
        writeln!(f, "  user: {:?}", self.nsinodes.user)?;
        writeln!(f, "  mnt:  {:?}", self.nsinodes.mnt)?;
        writeln!(f, "  cgroup:  {:?}", self.nsinodes.cgroup)?;
        writeln!(f, "  time:  {:?}", self.nsinodes.time)?;
        writeln!(f, "  time_for_children:  {:?}", self.nsinodes.time_for_children)?;
        Ok(())
    }
}

fn get_namespace_inodes(pid: Pid) -> NamespaceInodes {
    let pid: String = match pid {
        Pid::Pid(pid) => pid.to_string(),
        Pid::SelfPid => "self".to_string(),
    };
    let nspath = format!("/proc/{}/ns", pid);

    let mut ns = NamespaceInodes {
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



pub fn get_namespaces(pid: Pid) -> Namespaces {

    match pid {
        Pid::Pid(_) =>
        {
            // go up the parent hierarchy until we find a parent with different namespaces
            let mut ppid = pid;
            let nsinodes = get_namespace_inodes(pid);
            loop {
                let candidate_ppid = get_ppid(ppid);
                match candidate_ppid {
                    None => break,
                    Some(candidate_ppid) =>
                    {
                        let ppid_nsinodes = get_namespace_inodes(candidate_ppid);
                        if nsinodes.equal_mnt_and_net(&ppid_nsinodes) {
                            ppid=candidate_ppid;
                        } else {
                            break;
                        }
                    }

                }
            }

            let nspath = format!("{}/ns", pid.path());
            let nsroot = format!("{}/ns", ppid.path());
            Namespaces {
                nspath: nspath,
                nsroot: nsroot,
                nsinodes: nsinodes,
            }
        },
        Pid::SelfPid =>
        {
            let nsinodes = get_namespace_inodes(pid);
            let nspath = format!("{}/ns", pid.path());
            Namespaces {
                nspath: nspath.clone(),
                nsroot: nspath,
                nsinodes: nsinodes,
            }
        },
    }
}

/// Runs a function inside the given network and mount namespaces.
/// Returns the child PID so the caller can `waitpid` on it.
pub fn run_in_net_and_mnt_namespace(ns: Namespaces, func: Box<dyn Fn()>) -> nix::Result<nix::unistd::Pid> {
    //Note: The child process is created with a single thread—the one that called fork().
    match unsafe { fork()? } {
        ForkResult::Parent { child } => {
            // Parent: return the PID of the child
            Ok(child)
        }
        ForkResult::Child => {
            // enter namespace
            let path: &Path = Path::new(ns.nsroot.as_str());
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
