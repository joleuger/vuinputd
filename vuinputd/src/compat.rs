use libc::{__s32, __u16, c_ulong, input_event};


#[repr(C)]
pub struct input_event_compat {
    pub input_event_sec: u32,
    pub input_event_usec: u32,
    pub type_: __u16,
    pub code: __u16,
    pub value: __s32,
}

// this is static for the architecture
pub fn compat_uses_64bit_time() -> bool {
    let uname = nix::sys::utsname::uname().unwrap();
    let arch = uname.machine().to_str().unwrap();

    match arch {
        "x86_64" => false,
        "ppc64" => false, // some setups still 32-bit time_t
        _ => true, // arm64, riscv64, s390x all use 64-bit
    }
}

pub fn map_to_64_bit(compat: &input_event_compat) -> input_event{
    let mut mapped: input_event = unsafe { std::mem::zeroed() };
    mapped.time.tv_sec=compat.input_event_sec.into();
    mapped.time.tv_usec=compat.input_event_usec.into();
    mapped.type_=compat.type_;
    mapped.code=compat.code;
    mapped.value=compat.value;

    mapped
}