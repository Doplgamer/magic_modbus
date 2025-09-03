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

use crate::utils::{ModbusReadCommand, ModbusWriteCommand};
use crossterm::event::Event;
use ratatui::{style::Style, text::Line};
use std::net::SocketAddr;
use strum::{Display, EnumIter, FromRepr};

pub enum Action {
    CEvent(Event),
    Tick,
    Render,
    ToModbus(ModbusCommandQueue),   // From App to Modbus
    FromModbus(ModbusCommandQueue), // From Modbus to App
    SuccessfulWrite,
    Connect(SocketAddr),
    ConnectionError(String),
    Disconnect,
    Error(String),
    PageRefresh,
}

pub enum ModbusCommandQueue {
    Read(Vec<ModbusReadCommand>),
    Write(Vec<ModbusWriteCommand>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellType {
    Coil(bool),
    Word(u16),
}

impl CellType {
    pub fn to_u16(self) -> u16 {
        match self {
            CellType::Coil(content) => {
                if content {
                    1
                } else {
                    0
                }
            }
            CellType::Word(content) => content,
        }
    }
}

#[derive(Default, Display)]
pub enum ConnectionStatus {
    Connected,
    #[default]
    NotConnected,
}

#[derive(Clone)]
pub enum AppMode {
    Main,
    Help,
    Popup(PopupType),
}

#[derive(Clone)]
pub enum PopupType {
    Connection,
    Edit,
    Error(String),
    Goto,
    SaveMacro(SaveMacroMode),
}

#[derive(Clone)]
pub enum SaveMacroMode {
    Main,
    OverwriteWarning,
    FileSaved,
}

#[derive(Default)]
pub enum CurrentFocus {
    #[default]
    Top,
    Bottom,
}

#[derive(
    Default, Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Display, FromRepr, EnumIter,
)]
pub enum SelectedTopTab {
    #[default]
    #[strum(to_string = "Coils")]
    Coils,
    #[strum(to_string = "Discrete Inputs")]
    DiscreteInputs,
    #[strum(to_string = "Input Registers")]
    InputRegisters,
    #[strum(to_string = "Holding Registers")]
    HoldingRegisters,
}

impl SelectedTopTab {
    pub fn next(self) -> Self {
        let current_index = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
    }

    pub fn previous(self) -> Self {
        let current_index = self as usize;
        let previous_index = current_index.saturating_sub(1);
        Self::from_repr(previous_index).unwrap_or(self)
    }

    pub fn title(self) -> Line<'static> {
        Line::styled(format!("  {self}  "), Style::default())
    }
}

#[derive(Default, Clone, Copy, Display, FromRepr, EnumIter)]
pub enum SelectedBottomTab {
    #[default]
    #[strum(to_string = "Connection")]
    Connection,
    #[strum(to_string = "Queue")]
    Queue,
}

impl SelectedBottomTab {
    pub fn next(self) -> Self {
        let current_index = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
    }

    pub fn previous(self) -> Self {
        let current_index = self as usize;
        let previous_index = current_index.saturating_sub(1);
        Self::from_repr(previous_index).unwrap_or(self)
    }

    pub fn title(self) -> Line<'static> {
        Line::styled(format!("  {self}  "), Style::default())
    }
}

pub enum SelectedConnectionButton {
    NewConnection,
    Disconnect,
}

#[derive(Clone)]
pub enum CellState {
    Normal,
    Queued,
}

pub enum ConnectingField {
    Address,
    Port,
}
