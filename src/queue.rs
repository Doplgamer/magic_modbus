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

use crate::app_table::TableCell;
use crate::enums::{CellType, SelectedTopTab};

pub struct QueueItem {
    pub address: u16,
    pub cell: TableCell,
    pub table_index: usize,
}

impl QueueItem {
    pub fn table_name(&self) -> String {
        self.cell.table_type.to_string()
    }

    pub fn memory_address(&self) -> String {
        match self.cell.table_type {
            SelectedTopTab::Coils => format!("0x0{:04X}", self.address),
            SelectedTopTab::DiscreteInputs => format!("0x1{:04X}", self.address),
            SelectedTopTab::InputRegisters => format!("0x3{:04X}", self.address),
            SelectedTopTab::HoldingRegisters => format!("0x4{:04X}", self.address),
        }
    }

    pub fn original_content(&self) -> String {
        format!(
            "{:05}",
            match self.cell.original_content {
                CellType::Coil(content) => {
                    match content {
                        true => 1,
                        false => 0,
                    }
                }
                CellType::Word(content) => content,
            }
        )
    }

    pub fn queued_content(&self) -> String {
        format!(
            "{:05}",
            match self.cell.queued_content {
                CellType::Coil(content) => {
                    match content {
                        true => 1,
                        false => 0,
                    }
                }
                CellType::Word(content) => content,
            }
        )
    }
}
