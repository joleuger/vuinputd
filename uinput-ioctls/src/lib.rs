// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use libc::{c_char, c_uint};
use libc::{uinput_abs_setup, uinput_ff_erase, uinput_ff_upload, uinput_setup};

use nix::{
    ioctl_none, ioctl_read, ioctl_read_buf, ioctl_readwrite, ioctl_write_int, ioctl_write_ptr,
    request_code_none, request_code_read, request_code_readwrite, request_code_write,
};

pub const UI_DEV_CREATE: u64 = request_code_none!(b'U', 1);
pub const UI_DEV_DESTROY: u64 = request_code_none!(b'U', 2);
pub const UI_DEV_SETUP: u64 = request_code_write!(b'U', 3, ::std::mem::size_of::<uinput_setup>());
pub const UI_ABS_SETUP: u64 =
    request_code_write!(b'U', 4, ::std::mem::size_of::<uinput_abs_setup>());
//pub const UI_ABS_SETUP_WITHOUT_SIZE: u64 = request_code_write!(b'U', 4, 0);

pub const UI_GET_SYSNAME_WITHOUT_SIZE: u64 = request_code_read!(b'U', 44, 0);
//#define UI_GET_SYSNAME(len)	_IOC(_IOC_READ, UINPUT_IOCTL_BASE, 44, len)
pub const UI_GET_VERSION: u64 = request_code_read!(b'U', 45, ::std::mem::size_of::<c_uint>());

pub const UI_SET_EVBIT: u64 = request_code_write!(b'U', 100, std::mem::size_of::<c_uint>());
pub const UI_SET_KEYBIT: u64 = request_code_write!(b'U', 101, std::mem::size_of::<c_uint>());
pub const UI_SET_RELBIT: u64 = request_code_write!(b'U', 102, std::mem::size_of::<c_uint>());
pub const UI_SET_ABSBIT: u64 = request_code_write!(b'U', 103, std::mem::size_of::<c_uint>());
pub const UI_SET_MSCBIT: u64 = request_code_write!(b'U', 104, std::mem::size_of::<c_uint>());
pub const UI_SET_LEDBIT: u64 = request_code_write!(b'U', 105, std::mem::size_of::<c_uint>());
pub const UI_SET_SNDBIT: u64 = request_code_write!(b'U', 106, std::mem::size_of::<c_uint>());
pub const UI_SET_FFBIT: u64 = request_code_write!(b'U', 107, std::mem::size_of::<c_uint>());
pub const UI_SET_PHYS: u64 = request_code_write!(b'U', 108, ::std::mem::size_of::<*mut c_char>());
pub const UI_SET_SWBIT: u64 = request_code_write!(b'U', 109, std::mem::size_of::<c_uint>());
pub const UI_SET_PROPBIT: u64 = request_code_write!(b'U', 110, std::mem::size_of::<c_uint>());

pub const UI_BEGIN_FF_UPLOAD: u64 =
    request_code_readwrite!(b'U', 200, ::std::mem::size_of::<uinput_ff_upload>());
pub const UI_END_FF_UPLOAD: u64 =
    request_code_write!(b'U', 201, ::std::mem::size_of::<uinput_ff_upload>());
pub const UI_BEGIN_FF_ERASE: u64 =
    request_code_readwrite!(b'U', 202, ::std::mem::size_of::<uinput_ff_erase>());
pub const UI_END_FF_ERASE: u64 =
    request_code_write!(b'U', 203, ::std::mem::size_of::<uinput_ff_erase>());

ioctl_none!(ui_dev_create, b'U', 1);
ioctl_none!(ui_dev_destroy, b'U', 2);
ioctl_write_ptr! {ui_dev_setup, b'U', 3, uinput_setup}
ioctl_write_ptr! { ui_abs_setup, b'U', 4, uinput_abs_setup}

ioctl_read_buf! { ui_get_sysname, b'U', 44, c_char }
ioctl_read! { ui_get_version, b'U', 45, c_uint }

ioctl_write_int!(ui_set_evbit, b'U', 100);
ioctl_write_int!(ui_set_keybit, b'U', 101);
ioctl_write_int!(ui_set_relbit, b'U', 102);
ioctl_write_int!(ui_set_absbit, b'U', 103);
ioctl_write_int!(ui_set_mscbit, b'U', 104);
ioctl_write_int!(ui_set_ledbit, b'U', 105);
ioctl_write_int!(ui_set_sndbit, b'U', 106);
ioctl_write_int!(ui_set_ffbit, b'U', 107);
ioctl_write_ptr!(ui_set_phys, b'U', 108, *const c_char); // original macro #define UI_SET_PHYS _IOW(UINPUT_IOCTL_BASE, 108, char*)
ioctl_write_int!(ui_set_swbit, b'U', 109);
ioctl_write_int!(ui_set_propbit, b'U', 110);

ioctl_readwrite!(ui_begin_ff_upload, b'U', 200, uinput_ff_upload);
ioctl_write_ptr!(ui_end_ff_upload, b'U', 201, uinput_ff_upload);
ioctl_readwrite!(ui_begin_ff_erase, b'U', 202, uinput_ff_erase);
ioctl_write_ptr!(ui_end_ff_erase, b'U', 203, uinput_ff_erase);
