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
use std::io::Cursor;
use tokio::io::AsyncReadExt;

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

pub struct BufReader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> BufReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    pub async fn read_u8(&mut self) -> std::io::Result<u8> {
        self.cursor.read_u8().await
    }

    pub async fn read_u16(&mut self) -> std::io::Result<u16> {
        self.cursor.read_u16().await
    }

    pub async fn read_u32(&mut self) -> std::io::Result<u32> {
        self.cursor.read_u32().await
    }

    pub async fn read_exact(&mut self, len: usize) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0; len];
        self.cursor.read_exact(&mut buf).await?;
        Ok(buf)
    }
}
