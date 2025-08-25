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

use ratatui::style::{Color, palette::tailwind};
pub const PALETTES: [tailwind::Palette; 4] = [
    tailwind::ROSE,    // Coil Outputs
    tailwind::AMBER,   // Digital Outputs
    tailwind::EMERALD, // Analog Inputs
    tailwind::INDIGO,  // Holding Registers
];

pub struct AppColors {
    pub connection_connected_fg: Color,
    pub connection_not_selected_fg: Color,

    pub section_selected_fg: Color,
    pub section_unselected_fg: Color,

    pub table_normal_cell_bg: Color,
    pub table_alt_cell_bg: Color,
    pub table_unselected_normal_cell_bg: Color,
    pub table_unselected_alt_cell_bg: Color,
    pub table_selected_cell_fg: Color,
    pub table_unselected_cell_fg: Color,
}

impl AppColors {
    pub const fn new(color: &tailwind::Palette) -> Self {
        Self {
            connection_connected_fg: tailwind::GREEN.c400,
            connection_not_selected_fg: tailwind::SLATE.c600,

            section_selected_fg: color.c500,
            section_unselected_fg: tailwind::SLATE.c600,

            table_normal_cell_bg: color.c950,
            table_alt_cell_bg: color.c900,
            table_unselected_normal_cell_bg: tailwind::SLATE.c800,
            table_unselected_alt_cell_bg: tailwind::SLATE.c700,
            table_selected_cell_fg: color.c400,
            table_unselected_cell_fg: tailwind::SLATE.c500,
        }
    }
}
