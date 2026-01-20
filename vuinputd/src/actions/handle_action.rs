// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use super::action::Action;
use crate::input_realizer::input_device;
use crate::input_realizer::netlink_message;
use crate::input_realizer::runtime_data;

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
        Action::WriteUdevRuntimeData {
            runtime_data,
            major,
            minor,
        } => {
            runtime_data::ensure_udev_structure("/run",true)?;
            match runtime_data {
                Some(data) => runtime_data::write_udev_data("/run",&data, major.into(), minor.into())?,
                None => runtime_data::delete_udev_data("/run",major.into(), minor.into())?,
            }
            Ok(())
        }
        Action::EmitNetlinkMessage { netlink_message } => {
            netlink_message::send_udev_monitor_message_with_properties(netlink_message);
            Ok(())
        }
        Action::RemoveDevice { path, major, minor } => {
            input_device::remove_input_device(path, major.into(), minor.into())?;
            Ok(())
        }
    }
}
