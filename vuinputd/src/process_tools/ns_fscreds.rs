// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

use crate::process_tools::Pid;

#[derive(Debug, Clone, PartialEq)]
struct IdMapEntry {
    pub inside_start: u64,
    pub outside_start: u64,
    pub length: u64,
}

fn parse_id_map(pid: u32, map_type: &str) -> io::Result<Vec<IdMapEntry>> {
    let path = format!("/proc/{}/{}", pid, map_type);
    let file = fs::File::open(&path)?;
    let reader = io::BufReader::new(file);

    Ok(reader
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            let mut parts = line.split_whitespace();
            let inside_start = parts.next()?.parse().ok()?;
            let outside_start = parts.next()?.parse().ok()?;
            let length = parts.next()?.parse().ok()?;
            Some(IdMapEntry {
                inside_start,
                outside_start,
                length,
            })
        })
        .collect())
}

fn to_host_id(entries: &[IdMapEntry], inside_id: u64) -> Option<u64> {
    entries.iter().find_map(|e| {
        if inside_id >= e.inside_start && inside_id < e.inside_start + e.length {
            Some(e.outside_start + (inside_id - e.inside_start))
        } else {
            None
        }
    })
}

/// Returns the host UID that corresponds to `ns_uid` (e.g. 0) inside the container.
pub fn get_uid_in_container(pid: Pid, ns_uid: u64) -> anyhow::Result<u32> {
    let Pid::Pid(pid) = pid;
    let entries = parse_id_map(pid, "uid_map")?;
    to_host_id(&entries, ns_uid)
        .map(|id| id as u32)
        .ok_or_else(|| anyhow::anyhow!("uid {} is not mapped in /proc/{}/uid_map", ns_uid, pid))
}

/// Returns the host GID that corresponds to `ns_gid` (e.g. 0) inside the container.
pub fn get_gid_in_container(pid: Pid, ns_gid: u64) -> anyhow::Result<u32> {
    let Pid::Pid(pid) = pid;
    let entries = parse_id_map(pid, "gid_map")?;
    to_host_id(&entries, ns_gid)
        .map(|id| id as u32)
        .ok_or_else(|| anyhow::anyhow!("gid {} is not mapped in /proc/{}/gid_map", ns_gid, pid))
}

/// Switch filesystem UID/GID to the given host IDs.
/// GID must be set before UID — dropping UID=0 removes the ability to change GID.
pub fn acquire_uid_and_gid(target_uid: u32, target_gid: u32) -> anyhow::Result<()> {
    unsafe {
        libc::setfsgid(target_gid as libc::gid_t);
        libc::setfsuid(target_uid as libc::uid_t);
    }
    Ok(())
}

/// Switch filesystem UID/GID to match whatever owner the given path has on the host.
pub fn acquire_uid_and_gid_of_path(path: &str) -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(Path::new(path))?;
    let target_uid = metadata.uid();
    let target_gid = metadata.gid();
    acquire_uid_and_gid(target_uid, target_gid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(s: &str) -> Vec<IdMapEntry> {
        s.lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let inside_start = parts.next()?.parse().ok()?;
                let outside_start = parts.next()?.parse().ok()?;
                let length = parts.next()?.parse().ok()?;
                Some(IdMapEntry {
                    inside_start,
                    outside_start,
                    length,
                })
            })
            .collect()
    }

    #[test]
    fn uid0_in_rootless_container_maps_to_host_uid() {
        // Typical rootless setup: container root (0) → host uid 100000
        let map = parse_str("0 100000 65536");
        assert_eq!(to_host_id(&map, 0), Some(100000));
        assert_eq!(to_host_id(&map, 1), Some(100001));
    }

    #[test]
    fn uid_outside_range_returns_none() {
        let map = parse_str("0 100000 65536");
        assert_eq!(to_host_id(&map, 65536), None);
    }

    #[test]
    fn identity_map_returns_same_id() {
        // Process not in a user namespace: 0 0 4294967295
        let map = parse_str("0 0 4294967295");
        assert_eq!(to_host_id(&map, 0), Some(0));
        assert_eq!(to_host_id(&map, 1000), Some(1000));
    }

    #[test]
    fn proc_self_uid_is_parseable() {
        let uid = unsafe { libc::getuid() } as u64;
        let entries =
            parse_id_map(std::process::id(), "uid_map").expect("failed to read /proc/self/uid_map");
        assert!(
            to_host_id(&entries, uid).is_some(),
            "current uid {} not found in uid_map",
            uid
        );
    }
}
