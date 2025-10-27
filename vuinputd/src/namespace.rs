// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use log::debug;
use nix::{
    sched::{setns, CloneFlags},
    unistd::{fork, ForkResult, Pid},
};
use std::{
    fs::{self, File},
    os::fd::AsFd, path::{self, Path},
};

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Namespaces {
    pub nspath: String,
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

impl Namespaces {
    pub fn equal_mnt_and_net(&self, other: &Namespaces) -> bool {
        self.mnt == other.mnt && self.net == other.net
    }
}

impl std::fmt::Display for Namespaces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Namespaces:")?;
        writeln!(f, "  net:  {:?}", self.net)?;
        writeln!(f, "  uts:  {:?}", self.uts)?;
        writeln!(f, "  ipc:  {:?}", self.ipc)?;
        writeln!(f, "  pid:  {:?}", self.pid)?;
        writeln!(f, "  pid_for_children:  {:?}", self.pid_for_children)?;
        writeln!(f, "  user: {:?}", self.user)?;
        writeln!(f, "  mnt:  {:?}", self.mnt)?;
        writeln!(f, "  cgroup:  {:?}", self.cgroup)?;
        writeln!(f, "  time:  {:?}", self.time)?;
        writeln!(f, "  time_for_children:  {:?}", self.time_for_children)?;
        Ok(())
    }
}

pub fn get_namespaces(pid: Option<i32>) -> Namespaces {
    let pid: String = match pid {
        Some(pid) => pid.to_string(),
        None => "self".to_string(),
    };
    let nspath = format!("/proc/{}/ns", pid);

    let mut ns = Namespaces {
        nspath: nspath.clone(),
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

/// Runs a function inside the given network and mount namespaces.
/// Returns the child PID so the caller can `waitpid` on it.
pub fn run_in_net_and_mnt_namespace(ns: Namespaces, func: Box<dyn Fn()>) -> nix::Result<Pid> {
    //Note: The child process is created with a single thread—the one that called fork().
    match unsafe { fork()? } {
        ForkResult::Parent { child } => {
            // Parent: return the PID of the child
            Ok(child)
        }
        ForkResult::Child => {
            // enter namespace
            let path: &Path = Path::new(ns.nspath.as_str());
            if !fs::exists(path).unwrap() {
                debug!("the process whose namespaces we want to enter does not exist anymore!");
                std::process::exit(0);
            }
            let net = File::open(ns.nspath.clone() + "/net").expect("net not found");
            let mnt = File::open(ns.nspath.clone() + "/mnt").expect("mnt not found");
            setns(net.as_fd(), CloneFlags::CLONE_NEWNET).expect("couldn't enter net");
            setns(mnt.as_fd(), CloneFlags::CLONE_NEWNS).expect("couldn't enter mnt");
            // execute your function
            func();
            std::process::exit(0);
        }
    }
}
