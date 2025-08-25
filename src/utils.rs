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

use crate::enums::{CellType, SelectedTopTab};
use ratatui::layout::{Constraint, Layout, Rect};

pub type ModbusReadCommand = (SelectedTopTab, u16, u16); // Table, Starting Address, Address Count
pub type ModbusWriteCommand = (SelectedTopTab, u16, CellType); // Table, Table Address, Content

pub fn centered_rect(length_x: u16, length_y: u16, rect: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(length_y),
        Constraint::Fill(1),
    ])
    .split(rect)[1];

    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(length_x),
        Constraint::Fill(1),
    ])
    .split(vertical)[1]
}

pub fn trim_borders(rect: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(rect)[1];

    Layout::horizontal([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(vertical)[1]
}
