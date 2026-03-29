// SPDX-License-Identifier: MIT
//
// Author: Johannes Leupolz <dev@leupolz.eu>

use clap::{Parser, Subcommand};
use vuinputd_tests::scenarios::{
    basic_keyboard::BasicKeyboard, basic_mouse::BasicMouse, basic_ps4_gamepad::BasicPs4Gamepad,
    basic_xbox_gamepad::BasicXboxGamepad,
    ff_xbox_gamepad::FfXboxGamepad, /*
                                    reuse_keyboard::ReuseKeyboard, reuse_xbox_gamepad::ReuseXboxGamepad,
                                    ScenarioArgs, stress_keyboard::StressKeyboard, stress_xbox_gamepad::StressXboxGamepad, */
    ScenarioArgs,
};

#[derive(Parser)]
#[command(name = "test-scenarios")]
#[command(about = "Test scenarios for vuinputd", long_about = None)]
struct Cli {
    /// Run scenarios in IPC mode (communicate with vuinputd daemon)
    #[arg(short, long, default_value_t = false)]
    ipc: bool,

    /// Path to uinput device
    #[arg(short, long, default_value = "/dev/uinput")]
    dev_path: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Basic keyboard test
    BasicKeyboard,

    /// Basic mouse test
    BasicMouse,

    /// Basic PS4 gamepad test
    BasicPs4Gamepad,

    /// Basic Xbox gamepad test
    BasicXboxGamepad,

    /// Force feedback / Vibration Xbox gamepad test
    FfXboxGamepad,
    /*
    /// Reuse keyboard test (create, destroy, recreate)
    ReuseKeyboard,

    /// Reuse Xbox gamepad test (create, destroy, recreate)
    ReuseXboxGamepad,

    /// Stress test for keyboard (3000 events)
    StressKeyboard,

    /// Stress test for Xbox gamepad (3000 events)
    StressXboxGamepad,
     */
}

fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();

    let args = ScenarioArgs {
        ipc: cli.ipc,
        dev_path: Some(cli.dev_path),
    };

    match cli.command {
        Commands::BasicKeyboard => BasicKeyboard::run(&args),
        Commands::BasicMouse => BasicMouse::run(&args),
        Commands::BasicPs4Gamepad => BasicPs4Gamepad::run(&args),
        Commands::BasicXboxGamepad => BasicXboxGamepad::run(&args),
        Commands::FfXboxGamepad => FfXboxGamepad::run(&args),
        /*
        Commands::ReuseKeyboard => ReuseKeyboard::run(&args),
        Commands::ReuseXboxGamepad => ReuseXboxGamepad::run(&args),
        Commands::StressKeyboard => StressKeyboard::run(&args),
        Commands::StressXboxGamepad => StressXboxGamepad::run(&args),
         */
    }
}
