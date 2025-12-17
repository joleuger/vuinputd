// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use super::action::Action;
use super::input_device;
use super::netlink_message;
use super::runtime_data;

pub fn handle_cli_action(json: String) -> i32 {
    let action: Action = serde_json::from_str(&json).expect("invalid action JSON");
    handle_action(action).unwrap_or_else(|err| {
        panic!("Error handling action: {}", err);
    });
    0
}

fn handle_action(action: Action) -> anyhow::Result<()> {
    match action {
        Action::MknodDevice { path, major, minor } => {
            input_device::ensure_input_device(path, major.into(), minor.into())?;
            Ok(())
        }
        Action::EmitUdevEvent {
            netlink_message,
            runtime_data,
            major,
            minor,
        } => {
            netlink_message::send_udev_monitor_message_with_properties(netlink_message);
            runtime_data::ensure_udev_structure()?;
            match runtime_data {
                Some(data) => runtime_data::write_udev_data(&data, major.into(), minor.into())?,
                None => runtime_data::delete_udev_data(major.into(), minor.into())?,
            }
            Ok(())
        }
        Action::RemoveDevice { path, major, minor } => {
            input_device::remove_input_device(path, major.into(), minor.into())?;
            Ok(())
        }
    }
}
