// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use super::action::Action;

pub fn handle_cli_action(json: String) -> i32 {
    let action: Action = serde_json::from_str(&json).expect("invalid action JSON");
    handle_action(action).unwrap_or_else(|err| {
        panic!("Error handling action: {}", err);
    });
    0
}

fn handle_action(action: Action) -> Result<(), String> {
    match action {
        Action::MknodDevice { .. } => Ok(()),
        Action::AnnounceViaNetlink { .. } => Ok(()),
        Action::RemoveDevice { .. } => Ok(()),
    }
}
