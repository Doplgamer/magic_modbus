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

use crate::{
    enums::{Action, CellState, CellType, SelectedTopTab},
    queue::QueueItem,
};
use ratatui::widgets::TableState;
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub struct TableCell {
    pub original_content: CellType,
    pub queued_content: CellType,
    pub state: CellState,
    pub table_type: SelectedTopTab,
}

impl TableCell {
    pub fn new(table_type: SelectedTopTab) -> Self {
        Self {
            original_content: match table_type {
                SelectedTopTab::Coils | SelectedTopTab::DiscreteInputs => CellType::Coil(false),
                SelectedTopTab::InputRegisters | SelectedTopTab::HoldingRegisters => {
                    CellType::Word(0)
                }
            },
            queued_content: match table_type {
                SelectedTopTab::Coils | SelectedTopTab::DiscreteInputs => CellType::Coil(false),
                SelectedTopTab::InputRegisters | SelectedTopTab::HoldingRegisters => {
                    CellType::Word(0)
                }
            },
            state: CellState::Normal,
            table_type,
        }
    }

    pub fn apply(&mut self) {
        self.original_content = self.queued_content;
        self.state = CellState::Normal
    }

    fn queue(&mut self, new_value: CellType) {
        self.queued_content = new_value;
        self.state = if self.queued_content == self.original_content {
            CellState::Normal
        } else {
            CellState::Queued
        }
    }

    pub fn revert(&mut self) {
        self.queued_content = self.original_content;
        self.state = CellState::Normal
    }

    fn set(&mut self, new_value: CellType) {
        match self.state {
            CellState::Normal => {
                self.original_content = new_value;
                self.queued_content = new_value;
            }
            CellState::Queued => {
                self.original_content = new_value;
            }
        }
    }

    fn toggle(&mut self) {
        // Used for coils, not words
        match self.queued_content {
            CellType::Coil(content) => {
                self.queued_content = match content {
                    true => CellType::Coil(false),
                    false => CellType::Coil(true),
                }
            }
            CellType::Word(_) => {}
        }
        self.state = if self.queued_content == self.original_content {
            CellState::Normal
        } else {
            CellState::Queued
        }
    }
}

#[derive(Clone)]
pub struct AppTable {
    pub table_rows: usize,
    pub table_cols: usize,
    pub table_state: TableState,
    pub table_address: u16,
    pub table_type: SelectedTopTab,
    pub total_address_space: usize,
    pub data: HashMap<u16, TableCell>,
    pub page_offset: usize,
    pub sender: Sender<Action>,
}

impl AppTable {
    pub fn new(sender: Sender<Action>, table_type: SelectedTopTab) -> Self {
        AppTable {
            table_rows: 8,
            table_cols: match table_type {
                SelectedTopTab::Coils | SelectedTopTab::DiscreteInputs => 16,
                SelectedTopTab::InputRegisters | SelectedTopTab::HoldingRegisters => 8,
            },
            table_state: TableState::default().with_selected_cell(Some((0, 0))),
            table_address: 0,
            table_type,
            total_address_space: 65535, // 1 - 65535
            data: HashMap::new(),
            page_offset: 0,
            sender,
        }
    }

    pub fn get_visible_data(&self, start_index: u16, end_index: u16) -> Vec<TableCell> {
        (start_index..=end_index)
            .map(|index| {
                self.data
                    .get(&(index))
                    .cloned()
                    .unwrap_or_else(|| TableCell::new(self.table_type))
            })
            .collect::<Vec<TableCell>>()
    }

    pub fn page_size(&self) -> usize {
        self.table_rows * self.table_cols
    }

    pub async fn page_up(&mut self) {
        let (last_page_offset, _last_row, _last_col) = self.last_cell();
        if self.page_offset > 0 {
            self.page_offset -= 1;
        } else {
            self.page_offset = last_page_offset;
            if last_page_offset == 0 {
                self.go_to_existing_cell();
            }
        }
        self.go_to_existing_cell();
        self.set_memory_address(self.current_cell_index() as u16);

        let _ = self.sender.send(Action::PageRefresh).await;
    }

    pub async fn move_up(&mut self) {
        let (selected_row, _) = self.table_state.selected_cell().unwrap_or((0, 0));
        if selected_row > 0 {
            self.table_state.select_previous();
        } else {
            self.page_up().await;
            self.table_state.select(Some(self.table_rows - 1));
            self.go_to_existing_cell();
        }
        self.set_memory_address(self.current_cell_index() as u16);
    }

    pub async fn page_down(&mut self) {
        if (self.page_offset + 1) * self.page_size() < self.total_address_space {
            self.page_offset += 1;
            self.go_to_existing_cell();
        } else {
            self.page_offset = 0;
        }
        self.set_memory_address(self.current_cell_index() as u16);

        let _ = self.sender.send(Action::PageRefresh).await;
    }

    pub async fn move_down(&mut self) {
        let (selected_row, selected_col) = self.table_state.selected_cell().unwrap_or((0, 0));
        let (last_page_offset, last_row, _last_col) = self.last_cell();
        if selected_row < (self.table_rows - 1) {
            if selected_row != last_row || self.page_offset != last_page_offset {
                self.table_state.select_next();
                if !self.cell_exists(self.page_offset, selected_row + 1, selected_col) {
                    self.go_to_existing_cell();
                }
            } else {
                self.page_down().await;
                if last_page_offset == 0 {
                    self.table_state.select(Some(0));
                }
            }
        } else {
            self.page_down().await;
            self.table_state.select(Some(0));
            self.go_to_existing_cell();
        }
        self.set_memory_address(self.current_cell_index() as u16);
    }

    pub fn move_left(&mut self) {
        let selected_column = self.table_state.selected_column().unwrap_or(0);
        if selected_column > 0 {
            self.table_state.select_previous_column();
        } else {
            self.table_state.select_column(Some(self.table_cols - 1));
            self.go_to_existing_cell();
        }
        self.set_memory_address(self.current_cell_index() as u16);
    }

    pub fn move_right(&mut self) {
        let (selected_row, selected_col) = self.table_state.selected_cell().unwrap_or((0, 0));
        if selected_col < (self.table_cols - 1)
            && self.cell_exists(self.page_offset, selected_row, selected_col + 1)
        {
            self.table_state.select_next_column();
        } else {
            self.table_state.select_column(Some(0));
        }
        self.set_memory_address(self.current_cell_index() as u16); // Needs to be calculated
    }

    pub fn go_to_cell(&mut self, cell_address: u16) {
        let (page_offset, row, col) = self.index_to_cell(cell_address as usize);
        self.page_offset = page_offset;
        self.table_state.select_cell(Some((row, col)));
        self.set_memory_address(cell_address);
    }

    pub fn queue_current_cell(&mut self, new_value: CellType) {
        let current_index = self.current_cell_index();
        let cell = self
            .data
            .entry(current_index as u16)
            .or_insert(TableCell::new(self.table_type));
        cell.queue(new_value);
    }

    pub fn get_queue_items(&self) -> Vec<QueueItem> {
        self.data
            .iter()
            .filter_map(|(key, value)| {
                if let CellState::Queued = value.state {
                    Some(QueueItem {
                        address: *key,
                        cell: value.clone(),
                        table_index: self.table_type as usize,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn set_cell(&mut self, cell_index: u16, new_value: CellType) {
        let cell = self
            .data
            .entry(cell_index)
            .or_insert(TableCell::new(self.table_type));
        cell.set(new_value);
    }

    pub fn revert_current_cell(&mut self) {
        let current_index = self.current_cell_index();
        if let Some(cell) = self.data.get_mut(&(current_index as u16)) {
            cell.revert();
        }
    }

    pub fn toggle_current_coil(&mut self) {
        let current_index = self.current_cell_index();
        let cell = self
            .data
            .entry(current_index as u16)
            .or_insert(TableCell::new(self.table_type));
        cell.toggle();
    }

    fn cell_exists(&self, page_offset: usize, row: usize, col: usize) -> bool {
        self.cell_to_index(page_offset, row, col) < self.total_address_space
    }

    fn cell_to_index(&self, page_offset: usize, row: usize, col: usize) -> usize {
        (page_offset * self.page_size()) + (row * self.table_cols) + col
    }

    fn current_cell_exists(&self) -> bool {
        self.current_cell_index() < self.total_address_space
    }

    fn current_cell_index(&self) -> usize {
        self.cell_to_index(
            self.page_offset,
            self.table_state.selected().unwrap_or(0),
            self.table_state.selected_column().unwrap_or(0),
        )
    }

    fn go_to_existing_cell(&mut self) {
        let (_last_page_offset, last_row, last_col) = self.last_cell();
        if !self.current_cell_exists() {
            self.table_state.select(Some(last_row));
            if !self.current_cell_exists() {
                self.table_state.select_cell(Some((last_row, last_col)));
            }
        }
    }

    fn index_to_cell(&self, index: usize) -> (usize, usize, usize) {
        let page_offset = index / self.page_size();
        let row = (index / self.table_cols) % self.table_rows;
        let col = index % self.table_cols;

        (page_offset, row, col)
    }

    fn last_cell(&self) -> (usize, usize, usize) {
        let (page_offset, row, col) = self.index_to_cell(self.total_address_space - 1);
        (page_offset, row, col)
    }

    fn set_memory_address(&mut self, value: u16) {
        self.table_address = value;
    }
}
