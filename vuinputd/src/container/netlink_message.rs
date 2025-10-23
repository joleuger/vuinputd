use std::collections::HashMap;
use std::mem;
use std::os::fd::{AsRawFd, OwnedFd};

use std::io::{Cursor, IoSlice};

use log::debug;
use nix::sys::socket::{
    bind, sendmsg, socket, AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType
};

/// Netlink constants
pub const UDEV_EVENT_MODE: u32 = 2;
pub const UDEV_MONITOR_MAGIC: u32 = 0xfeedcafe;
pub const MAX_NETLINK_PAYLOAD: usize = 64 * 1024; // 64 KiB

// to test, use "udevadm --debug monitor -p"

// Taken from: https://github.com/systemd/systemd/blob/61afc53924dd3263e7b76b1323a5fe61d589ffd2/src/libsystemd/sd-device/device-monitor.c#L67-L86
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MonitorNetlinkHeader {
    pub prefix: [u8; 8],
    pub magic: u32,
    pub header_size: u32,
    pub properties_off: u32,
    pub properties_len: u32,
    pub filter_subsystem_hash: u32,
    pub filter_devtype_hash: u32,
    pub filter_tag_bloom_hi: u32,
    pub filter_tag_bloom_lo: u32,
}

impl MonitorNetlinkHeader {
    pub fn new(properties_len: usize, subsystem: Option<&str>, devtype: Option<&str>) -> Self {
        let mut prefix = [0u8; 8];
        // "libudev" plus null: matches original implementation
        prefix[..7].copy_from_slice(b"libudev");
        prefix[7] = 0;

        let mut hdr = Self {
            prefix,
            magic: UDEV_MONITOR_MAGIC.to_be(),
            header_size: mem::size_of::<MonitorNetlinkHeader>() as u32,
            properties_off: mem::size_of::<MonitorNetlinkHeader>() as u32,
            properties_len: properties_len as u32,
            filter_subsystem_hash: 0,
            filter_devtype_hash: 0,
            filter_tag_bloom_hi: 0,
            filter_tag_bloom_lo: 0,
        };

        if let Some(s) = subsystem {
            hdr.filter_subsystem_hash = string_hash32(s).to_be();
        }
        if let Some(d) = devtype {
            hdr.filter_devtype_hash = string_hash32(d).to_be();
        }

        hdr
    }

    /// Serialize header to bytes (safe copy)
    pub fn to_bytes(&self) -> Vec<u8> {
        // repr(C) fixed-size struct -> safe to transmute bytes by copying
        let ptr = self as *const MonitorNetlinkHeader as *const u8;
        unsafe { std::slice::from_raw_parts(ptr, mem::size_of::<MonitorNetlinkHeader>()).to_vec() }
    }
}

pub fn string_hash32(s: &str) -> u32 {
    // Note: needs to be compatible with https://github.com/systemd/systemd/blob/main/src/libudev/libudev-monitor.c
    // and https://gitlab.freedesktop.org/libinput/libinput/-/blob/main/src/udev-seat.c?ref_type=heads.
    // Because in our use case, only subsystem "input" is relevant, we just hard code the values from murmur hash 2.
    match s {
        "input" => 3248653424,
        "" => 0,
        _ => panic!("uncovered use case")
    }
}

/// Open netlink socket, bind to groups
fn open_netlink(groups: u32) -> Result<OwnedFd, String> {
    // Domain AF_NETLINK, type SOCK_RAW, protocol NETLINK_KOBJECT_UEVENT
    let fd = socket(
        AddressFamily::Netlink,
        SockType::Raw,
        SockFlag::empty(),
        SockProtocol::NetlinkKObjectUEvent,
    )
    .map_err(|e| format!("Could not create netlink socket: {}", e))?;

    // pid 0 => the kernel takes care of assigning it.
    let sockaddr=NetlinkAddr::new(0, groups);
    let raw_fd= fd.as_raw_fd();

    bind(raw_fd, &sockaddr).map_err(|e| {
        format!("Could not bind netlink socket: {}", e)
    })?;

    Ok(fd)
}

/// Send the monitor header + payload over NETLINK_KOBJECT_UEVENT.
/// - `payload` should be the raw udev-style `\0` separated key=value bytes (no base64)
/// - `subsystem`/`devtype` optionally used to compute filter hashes
pub fn send_udev_monitor_message(
    payload: &[u8],
    subsystem: Option<&str>,
    devtype: Option<&str>,
    groups: u32,
) -> Result<(), String> {
    if payload.len() + mem::size_of::<MonitorNetlinkHeader>() > MAX_NETLINK_PAYLOAD {
        return Err(format!(
            "Total payload too large: {} bytes (max {})",
            payload.len() + mem::size_of::<MonitorNetlinkHeader>(),
            MAX_NETLINK_PAYLOAD
        ));
    }

    let header = MonitorNetlinkHeader::new(payload.len(), subsystem, devtype);
    let header_bytes = header.to_bytes();

    let fd = open_netlink(groups)?;

    // prepare iovecs
    let iov = [
        IoSlice::new(&header_bytes),
        IoSlice::new(payload),
    ];

    // destination sockaddr (NULL nl_pid => kernel / multicast)
    let sockaddr = NetlinkAddr::new(0, groups);

    let _rc = sendmsg(fd.as_raw_fd(), &iov, &[], MsgFlags::empty(), Some(&sockaddr))
        .map_err(|e| format!("Could not send message: {}", e));
    debug!("udev message sent");

    // ensure cleanup
    drop(fd);

    Ok(())
}

pub fn send_udev_monitor_message_with_properties(properties:HashMap<String, String>) {
    let device_name = match properties.get("DEVNAME") {
        Some(name) => name,
        None => "unknown device"
    };
    debug!("Sending udev message over netlink for {}",device_name);
    let mut payload:Vec<u8> = Vec::new();
    for (key,value) in properties.iter() {
        payload.extend(key.as_bytes());
        payload.extend("=".as_bytes());
        payload.extend(value.as_bytes());
        payload.push(0);
    }
    
    send_udev_monitor_message(&payload,Some("input"),None,UDEV_EVENT_MODE).unwrap();
}

// println!("{:02X?}", payload);
// 746573743D76616C75650043555252454E545F544147533D3A736561745F7675696E7075743A00544147533D3A736561745F7675696E7075743A00444556504154483D2F646576696365732F7669727475616C2F696E7075742F696E7075743133382F6576656E74390049445F5655494E5055545F4D4F5553453D31004D494E4F523D37330049445F494E5055543D31002E494E5055545F434C4153533D6D6F757365005345514E554D3D3134393231002E484156455F485744425F50524F504552544945533D31004D414A4F523D313300414354494F4E3D6164640049445F53455249414C3D6E6F73657269616C004445564E414D453D2F6465762F696E7075742F6576656E743900555345435F494E495449414C495A45443D31373337373733353034373139320049445F5655494E5055543D310049445F534541543D736561745F7675696E7075740053554253595354454D3D696E70757400
// dGVzdD12YWx1ZQBDVVJSRU5UX1RBR1M9OnNlYXRfdnVpbnB1dDoAVEFHUz06c2VhdF92dWlucHV0OgBERVZQQVRIPS9kZXZpY2VzL3ZpcnR1YWwvaW5wdXQvaW5wdXQxMzgvZXZlbnQ5AElEX1ZVSU5QVVRfTU9VU0U9MQBNSU5PUj03MwBJRF9JTlBVVD0xAC5JTlBVVF9DTEFTUz1tb3VzZQBTRVFOVU09MTQ5MjEALkhBVkVfSFdEQl9QUk9QRVJUSUVTPTEATUFKT1I9MTMAQUNUSU9OPWFkZABJRF9TRVJJQUw9bm9zZXJpYWwAREVWTkFNRT0vZGV2L2lucHV0L2V2ZW50OQBVU0VDX0lOSVRJQUxJWkVEPTE3Mzc3NzM1MDQ3MTkyAElEX1ZVSU5QVVQ9MQBJRF9TRUFUPXNlYXRfdnVpbnB1dABTVUJTWVNURU09aW5wdXQA



/*
UDEV  [16427452.069342] add      /devices/virtual/input/input97 (input)
ACTION=add
DEVPATH=/devices/virtual/input/input97
SUBSYSTEM=input
PRODUCT=3/beef/dead/0
NAME="Example device"
PROP=0
EV=3
KEY=ffffffefffff fffffffffffffffe
MODALIAS=input:b0003vBEEFpDEADe0000-e0,1,kramlsfw
SEQNUM=14498
USEC_INITIALIZED=16427452066918
ID_VUINPUT_KEYBOARD=1
ID_INPUT=1
ID_INPUT_KEY=1
.INPUT_CLASS=kbd
ID_SERIAL=noserial
ID_SEAT=seat_vuinput
TAGS=:seat:
CURRENT_TAGS=:seat:

UDEV  [16427452.089779] add      /devices/virtual/input/input97/event9 (input)
ACTION=add
DEVPATH=/devices/virtual/input/input97/event9
SUBSYSTEM=input
DEVNAME=/dev/input/event9
SEQNUM=14499
USEC_INITIALIZED=16427452068006
ID_VUINPUT_KEYBOARD=1
.HAVE_HWDB_PROPERTIES=1
ID_INPUT=1
ID_INPUT_KEY=1
.INPUT_CLASS=kbd
ID_SERIAL=noserial
ID_SEAT=seat_vuinput
MAJOR=13
MINOR=73
TAGS=:seat_vuinput:power-switch:
CURRENT_TAGS=:seat_vuinput:power-switch:


*/