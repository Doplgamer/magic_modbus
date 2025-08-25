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
mod logger;
mod queue;
mod utils;

use crate::app::App;
use color_eyre::Result;

// TODO Eventually have a way to convert the queued command / current connection into a macro file
// Plugging this file into magic_modbus will connect to the specified place and run the commands
// Adding any extra arguments to the macro, such as the address/port to connect to, will change what's done, otherwise
// the program will fallback to the macro itself

// TODO Finish the help menu once all the controls are thought out

// TODO
// Make sure that the queue menu actually reflects the queued cells, make the macro mechanic work properly
// Make sure that the log menu actually logs what's going on, turn to that async logging that you checked out for a bit
// Add integration with Clap (auto-connecting to an address/port, using a saved macro, etc etc)
// Clean up the code to make it more organized
// Run it through Warp to check for any potential problems
// Get it on github with a good license to protect your work
// Make a post on LinkedIn

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();

    App::new().run(&mut terminal).await?;

    ratatui::restore();

    Ok(())
}
