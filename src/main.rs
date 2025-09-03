//!   Copyright 2025 Isaac Schlaegel
//!
//!    Licensed under the Apache License, Version 2.0 (the "License");
//!    you may not use this file except in compliance with the License.
//!    You may obtain a copy of the License at
//!
//!        http://www.apache.org/licenses/LICENSE-2.0
//!
//!    Unless required by applicable law or agreed to in writing, software
//!    distributed under the License is distributed on an "AS IS" BASIS,
//!    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//!    See the License for the specific language governing permissions and
//!    limitations under the License.

mod app;
mod app_colors;
mod app_table;
mod enums;
mod macro_parser;
mod queue;
mod utils;

use crate::{app::App, macro_parser::MagModCommandList};
use clap::{ArgGroup, Parser, Subcommand};
use color_eyre::Result;
use std::{net::IpAddr, path::PathBuf};

#[derive(Parser)]
#[command(version, about, author)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    #[arg(short, long, value_parser, requires = "port")]
    /// Target address
    address: Option<IpAddr>,
    #[arg(short, long, value_parser, requires = "address")]
    /// Target port
    port: Option<u16>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(group(
    ArgGroup::new("macro_file")
    .required(true)
    .multiple(false)
    .args(["macro_file_no_confirm", "macro_file_with_confirm"])
    ))]
    #[command(group(
    ArgGroup::new("run_mode")
    .required(false)
    .multiple(false)
    .args(["check_connection", "dry_run"])
    ))]
    /// Access the macro parser
    ParseMacro {
        #[arg(short = 'M')]
        /// Run macro file immediately
        macro_file_no_confirm: Option<PathBuf>,
        #[arg(short = 'm')]
        /// Allow changing of IP Address + Port
        macro_file_with_confirm: Option<PathBuf>,
        #[arg(long = "check-connection")]
        /// Check to see if `magic-modbus` can connect
        check_connection: bool,
        #[arg(long = "dry-run")]
        /// Simulate a connection without actually doing anything
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::ParseMacro {
            macro_file_with_confirm,
            macro_file_no_confirm,
            check_connection,
            dry_run,
        }) => {
            if let Some(file_path) = macro_file_with_confirm {
                let mut command_list = MagModCommandList::from_file(file_path).await?;
                command_list
                    .run_macro(true, check_connection, dry_run)
                    .await?;
            }

            if let Some(file_path) = macro_file_no_confirm {
                let mut command_list = MagModCommandList::from_file(file_path).await?;
                command_list
                    .run_macro(false, check_connection, dry_run)
                    .await?;
            }
        }
        None => {
            let mut terminal = ratatui::init();

            App::new().run(&mut terminal, cli.address, cli.port).await?;

            ratatui::restore();
        }
    }

    Ok(())
}
