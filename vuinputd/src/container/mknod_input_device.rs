use nix::sys::stat::{makedev, mknod, stat, Mode, SFlag};
use nix::unistd::{chown, Gid, Uid};
use std::error::Error;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

pub fn ensure_input_device(dev_path: String, major: u64, minor: u64) -> Result<(), Box<dyn Error>> {
    let path = Path::new(&dev_path);
    let expected_dev = makedev(major, minor);
    let expected_mode = 0o666;

    // --- Step 1: Ensure node correctness ---
    let needs_replacement = if path.exists() {
        match stat(path) {
            Ok(st) => {
                let is_char = (st.st_mode & libc::S_IFMT as u32) == libc::S_IFCHR as u32;
                let dev_ok = st.st_rdev == expected_dev;
                !(is_char && dev_ok)
            }
            Err(_) => true,
        }
    } else {
        true
    };

    if needs_replacement {
        println!("Replacing {}", dev_path);
        let _ = fs::remove_file(path);
        let mode = Mode::from_bits_truncate(expected_mode);
        mknod(path, SFlag::S_IFCHR, mode, expected_dev)?;
    } else {
        println!("{} is already correct device", dev_path);
    }

    // --- Step 2: Ensure ownership and permissions ---
    if let Ok(meta) = fs::metadata(path) {
        let perms = meta.permissions().mode() & 0o777;

        if perms != expected_mode {
            println!("Fixing mode of {} (was {:o})", dev_path, perms);
            fs::set_permissions(path, fs::Permissions::from_mode(expected_mode))?;
        }

        /* TODO: Think about it
        let expected_uid = 0;
        let expected_gid = 0;
        let uid = meta.uid();
        let gid = meta.gid();
        if uid != expected_uid || gid != expected_gid {
            println!(
                "Fixing ownership of {} (was uid={}, gid={})",
                dev_path, uid, gid
            );
            chown(path, Some(Uid::from_raw(expected_uid)), Some(Gid::from_raw(expected_gid)))?;
        }
         */
    }

    Ok(())
}

/*
fn main() {
    let name = "/dev/input/example0";
    let major = 13;        // example values
    let minor = 37;

    if let Err(e) = ensure_input_device(name, major, minor) {
        eprintln!("Error: {}", e);
    }
}
     */
