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

use std::{
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use color_eyre::Result;
use futures::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{Event, EventStream, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Tabs, Wrap,
    },
};
use strum::IntoEnumIterator;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};
use tokio_modbus::client::{Reader, Writer, tcp};
use tokio_util::sync::CancellationToken;

use crate::{
    app_colors::{AppColors, PALETTES},
    app_table::AppTable,
    enums::*,
    macro_parser::MagModCommandList,
    queue::QueueItem,
    utils::{ModbusReadCommand, ModbusWriteCommand, centered_rect, trim_borders},
};

const CONNECTION_POPUP_TEXT: &str = "Please Enter an IP Address and Port";

const FOOTER_TEXT: [&str; 6] = [
    "(Esc) Quit | (Q) Previous Tab | (E) Next Tab | (Tab) Change Focus | (?) Help", // Main Controls
    "(W A S D) Navigate | (Space) Toggle/Edit | (Enter) Apply | (G) Go To", // Top Tab Controls
    "(← →) Select Button | (Enter) Connect/Disconnect",                     // Connection Menu
    "(↑ ↓) Navigate | (G) Go To Address | (R) Revert Item | (M) Save Macro", // Queue Menu
    "(Enter) - Close Popup",                                                // Error Popup
    "Enter address (1-65535) | (Enter) Go To Address | (Esc) Cancel",       // Goto Popup
];

pub struct App {
    // Main Async Event Loop
    cancellation_token: CancellationToken,
    main_task: JoinHandle<()>,
    sender: Sender<Action>,
    receiver: Receiver<Action>,

    // Modbus Event Loop
    modbus_task: Option<JoinHandle<()>>,
    modbus_sender: Sender<ModbusCommandQueue>,

    // Networking
    connection_status: ConnectionStatus,
    current_ip_address: Option<Ipv4Addr>,
    current_port: Option<u16>,
    selected_connection_button: SelectedConnectionButton,

    // UI Focus
    app_mode: AppMode,
    current_focus: CurrentFocus,
    selected_bottom_tab: SelectedBottomTab,
    selected_top_tab: SelectedTopTab,

    // Tables + Colors
    colors: AppColors,
    tables: Vec<AppTable>,

    // Queue Tab
    queue_table_data: Vec<QueueItem>,
    queue_table_state: TableState,
    queue_item_index: usize,
    queue_scroll_state: ScrollbarState,

    // Connection Popup
    connecting_popup_field: ConnectingField,
    address_input_cursor: usize,
    address_input: String,
    port_input_cursor: usize,
    port_input: String,

    // Edit Popup
    edit_popup_cursor: usize,
    edit_popup_input: String,

    // Goto Popup
    goto_popup_cursor: usize,
    goto_popup_input: String,

    // Macro Popup
    macro_popup_cursor: usize,
    macro_popup_input: String,

    // Misc Statuses
    page_refresh: bool, // Reads the page every time you change pages
    tick_refresh: bool, // Reads the page every tick
    help_menu_page: u8,
    exit: bool,
}

impl App {
    pub fn new() -> App {
        let (sender, receiver) = mpsc::channel::<Action>(100);
        let (dummy_tx, _dummy_rx) = mpsc::channel::<ModbusCommandQueue>(1);
        App {
            // Async Event Loop
            cancellation_token: CancellationToken::new(),
            main_task: tokio::spawn(async {}),
            sender: sender.clone(),
            receiver,

            // Modbus Event Loop
            modbus_task: None,
            modbus_sender: dummy_tx,

            // Networking
            connection_status: ConnectionStatus::default(),
            current_ip_address: None,
            current_port: None,
            selected_connection_button: SelectedConnectionButton::NewConnection,

            // UI Focus
            app_mode: AppMode::Main,
            current_focus: CurrentFocus::default(),
            selected_bottom_tab: SelectedBottomTab::default(),
            selected_top_tab: SelectedTopTab::default(),

            // Tables + Colors
            colors: AppColors::new(&PALETTES[0]),
            tables: vec![
                AppTable::new(sender.clone(), SelectedTopTab::Coils),
                AppTable::new(sender.clone(), SelectedTopTab::DiscreteInputs),
                AppTable::new(sender.clone(), SelectedTopTab::InputRegisters),
                AppTable::new(sender.clone(), SelectedTopTab::HoldingRegisters),
            ],

            // Queue Tab
            queue_table_data: vec![],
            queue_table_state: TableState::new(),
            queue_item_index: 0,
            queue_scroll_state: ScrollbarState::new(1),

            // Connection Popup
            connecting_popup_field: ConnectingField::Address,
            address_input: String::from(" "),
            port_input: String::from(" "),
            address_input_cursor: 0,
            port_input_cursor: 0,

            // Edit Popup
            edit_popup_cursor: 0,
            edit_popup_input: String::new(),

            // Goto Popup
            goto_popup_cursor: 0,
            goto_popup_input: String::new(),

            // Macro Popup
            macro_popup_cursor: 0,
            macro_popup_input: String::new(),

            // Misc Statuses
            page_refresh: false,
            tick_refresh: false,
            help_menu_page: 0,
            exit: false,
        }
    }

    pub async fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
        addr: Option<IpAddr>,
        port: Option<u16>,
    ) -> Result<()> {
        self.cancellation_token.cancel();
        self.cancellation_token = CancellationToken::new();

        let event_sender = self.sender.clone();
        let cancel_token = self.cancellation_token.clone();

        // Main Event Loop
        self.main_task = tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick_interval = tokio::time::interval(Duration::from_secs(1));
            let mut render_interval = tokio::time::interval(Duration::from_secs_f64(1.0 / 60.0));

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => break,
                    maybe_event = reader.next() => {
                        match maybe_event {
                            Some(Ok(event)) => {
                                if event_sender.send(Action::CEvent(event)).await.is_err() {
                                    break;
                                }
                            },
                            Some(Err(_)) => {},
                            None => {
                                break;
                            },
                        }
                    },
                    _ = tick_interval.tick() => {
                        let _ = event_sender.send(Action::Tick).await;
                    },
                    _ = render_interval.tick() => {
                        let _ = event_sender.send(Action::Render).await;
                    }
                }
            }
        });

        if let (Some(addr), Some(port)) = (addr, port) {
            let socket_addr = SocketAddr::new(addr, port);
            let _ = self.sender.send(Action::Connect(socket_addr)).await;
        }

        while !self.exit {
            match self.receiver.recv().await {
                Some(action) => match action {
                    Action::CEvent(event) => self.on_crossterm_event(event).await?,
                    Action::Tick => {
                        if self.tick_refresh {
                            self.modbus_read_current_page().await;
                        }
                    }
                    Action::Render => {
                        terminal.draw(|frame| self.render(frame))?;
                    }
                    Action::ToModbus(queue) => {
                        let _ = self.modbus_sender.send(queue).await;
                    }
                    Action::FromModbus(queue) => {
                        if let ModbusCommandQueue::Write(commands) = queue {
                            self.apply_modbus_updates(commands);
                        }
                    }
                    Action::Connect(addr) => self.start_modbus_task(addr).await?,
                    Action::ConnectionError(message) => {
                        self.connection_status = ConnectionStatus::NotConnected;
                        self.current_ip_address = None;
                        self.current_port = None;

                        self.app_mode = AppMode::Popup(PopupType::Error(message));
                    }
                    Action::Disconnect => {
                        self.stop_modbus_task().await;
                    }
                    Action::Error(message) => {
                        self.app_mode = AppMode::Popup(PopupType::Error(message));
                    }
                    Action::PageRefresh => {
                        if self.page_refresh {
                            self.modbus_read_current_page().await;
                        }
                    }
                    Action::SuccessfulWrite => {
                        self.table_apply_queued_cells();
                    }
                },
                None => {
                    break;
                }
            }
        }

        self.stop()?;

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.cancellation_token.cancel();
        let mut counter = 0;
        while !self.main_task.is_finished() {
            std::thread::sleep(Duration::from_millis(1));
            counter += 1;
            if counter > 50 {
                self.main_task.abort();
            }
        }
        Ok(())
    }

    async fn start_modbus_task(&mut self, addr: SocketAddr) -> Result<()> {
        self.stop_modbus_task().await;

        let (tx_to_task, mut rx_from_ui) = mpsc::channel::<ModbusCommandQueue>(100);
        self.modbus_sender = tx_to_task.clone();

        self.connection_status = ConnectionStatus::Connected;
        self.current_ip_address = match addr.ip() {
            IpAddr::V4(v4) => Some(v4),
            _ => self.current_ip_address,
        };
        self.current_port = Some(addr.port());

        let ui_tx = self.sender.clone();

        self.modbus_task = Some(tokio::spawn(async move {
            let mut ctx = match tcp::connect(addr).await {
                Ok(c) => c,
                Err(e) => {
                    let _ = ui_tx.send(Action::ConnectionError(e.to_string())).await;
                    return;
                }
            };
            while let Some(queue) = rx_from_ui.recv().await {
                match queue {
                    ModbusCommandQueue::Read(commands) => {
                        let mut table_commands = Vec::new();
                        for (table, start, count) in commands {
                            match table {
                                SelectedTopTab::Coils => match ctx.read_coils(start, count).await {
                                    Ok(tcp_result) => match tcp_result {
                                        Ok(modbus_result) => {
                                            for (i, coil) in modbus_result.into_iter().enumerate() {
                                                table_commands.push((
                                                    table,
                                                    start + i as u16,
                                                    CellType::Coil(coil),
                                                ));
                                            }
                                        }
                                        Err(modbus_err) => {
                                            let _ = ui_tx
                                                .send(Action::Error(format!(
                                                    "Modbus Error: {}",
                                                    modbus_err
                                                )))
                                                .await;
                                        }
                                    },
                                    Err(_) => {
                                        let _ = ui_tx
                                            .send(Action::ConnectionError(String::from(
                                                "Connection Was Lost",
                                            )))
                                            .await;
                                    }
                                },
                                SelectedTopTab::DiscreteInputs => {
                                    match ctx.read_discrete_inputs(start, count).await {
                                        Ok(tcp_result) => match tcp_result {
                                            Ok(modbus_result) => {
                                                for (i, coil) in
                                                    modbus_result.into_iter().enumerate()
                                                {
                                                    table_commands.push((
                                                        table,
                                                        start + i as u16,
                                                        CellType::Coil(coil),
                                                    ));
                                                }
                                            }
                                            Err(modbus_err) => {
                                                let _ = ui_tx
                                                    .send(Action::Error(format!(
                                                        "Modbus Error: {}",
                                                        modbus_err
                                                    )))
                                                    .await;
                                            }
                                        },
                                        Err(_) => {
                                            let _ = ui_tx
                                                .send(Action::ConnectionError(String::from(
                                                    "Connection Was Lost",
                                                )))
                                                .await;
                                        }
                                    }
                                }
                                SelectedTopTab::InputRegisters => {
                                    match ctx.read_input_registers(start, count).await {
                                        Ok(tcp_result) => match tcp_result {
                                            Ok(modbus_result) => {
                                                for (i, word) in
                                                    modbus_result.into_iter().enumerate()
                                                {
                                                    table_commands.push((
                                                        table,
                                                        start + i as u16,
                                                        CellType::Word(word),
                                                    ));
                                                }
                                            }
                                            Err(modbus_err) => {
                                                let _ = ui_tx
                                                    .send(Action::Error(format!(
                                                        "Modbus Error: {}",
                                                        modbus_err
                                                    )))
                                                    .await;
                                            }
                                        },
                                        Err(_) => {
                                            let _ = ui_tx
                                                .send(Action::ConnectionError(String::from(
                                                    "Connection Was Lost",
                                                )))
                                                .await;
                                        }
                                    }
                                }
                                SelectedTopTab::HoldingRegisters => {
                                    match ctx.read_holding_registers(start, count).await {
                                        Ok(tcp_result) => match tcp_result {
                                            Ok(modbus_result) => {
                                                for (i, word) in
                                                    modbus_result.into_iter().enumerate()
                                                {
                                                    table_commands.push((
                                                        table,
                                                        start + i as u16,
                                                        CellType::Word(word),
                                                    ));
                                                }
                                            }
                                            Err(modbus_err) => {
                                                let _ = ui_tx
                                                    .send(Action::ConnectionError(format!(
                                                        "Modbus Error: {}",
                                                        modbus_err
                                                    )))
                                                    .await;
                                            }
                                        },
                                        Err(_) => {
                                            let _ = ui_tx
                                                .send(Action::ConnectionError(String::from(
                                                    "Connection Was Lost",
                                                )))
                                                .await;
                                        }
                                    }
                                }
                            }
                        }
                        let _ = ui_tx
                            .send(Action::FromModbus(ModbusCommandQueue::Write(
                                table_commands,
                            )))
                            .await;
                    }
                    ModbusCommandQueue::Write(commands) => {
                        let mut was_successful = true;
                        for command in commands {
                            let (table, addr, content) = command;
                            match (table, content) {
                                (SelectedTopTab::Coils, CellType::Coil(b)) => {
                                    if ctx.write_single_coil(addr, b).await.is_err() {
                                        let _ = ui_tx
                                            .send(Action::ConnectionError(String::from(
                                                "Connection Was Lost",
                                            )))
                                            .await;
                                        was_successful = false;
                                        break;
                                    }
                                }
                                (SelectedTopTab::HoldingRegisters, CellType::Word(w)) => {
                                    if ctx.write_single_register(addr, w).await.is_err() {
                                        let _ = ui_tx
                                            .send(Action::ConnectionError(String::from(
                                                "Connection Was Lost",
                                            )))
                                            .await;
                                        was_successful = false;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if was_successful {
                            let _ = ui_tx.send(Action::SuccessfulWrite).await;
                        }
                    }
                }
            }
        }));

        Ok(())
    }

    async fn stop_modbus_task(&mut self) {
        if let Some(handle) = self.modbus_task.take() {
            handle.abort();
        }

        let (dummy_tx, _dummy_rx) = mpsc::channel::<ModbusCommandQueue>(1);
        self.modbus_sender = dummy_tx;

        self.connection_status = ConnectionStatus::NotConnected;
        self.current_ip_address = None;
        self.current_port = None;
    }

    async fn on_crossterm_event(&mut self, event: Event) -> Result<()> {
        if let Event::Key(key) = event {
            if key.kind.is_press() {
                let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
                match &self.app_mode {
                    AppMode::Main => {
                        match self.current_focus {
                            CurrentFocus::Top => {
                                match key.code {
                                    KeyCode::Esc => self.exit = true,
                                    KeyCode::Tab => self.current_focus = CurrentFocus::Bottom,
                                    KeyCode::Char('q') => self.previous_top_tab(),
                                    KeyCode::Char('e') => self.next_top_tab(),
                                    KeyCode::Up | KeyCode::Char('w') if shift_pressed => {
                                        self.table_page_up().await
                                    }
                                    KeyCode::Up | KeyCode::Char('w') => self.table_move_up().await,
                                    KeyCode::Down | KeyCode::Char('s') if shift_pressed => {
                                        self.table_page_down().await
                                    }
                                    KeyCode::Down | KeyCode::Char('s') => {
                                        self.table_move_down().await
                                    }
                                    KeyCode::Left | KeyCode::Char('a') => self.table_move_left(),
                                    KeyCode::Right | KeyCode::Char('d') => self.table_move_right(),
                                    KeyCode::Char('r') => {
                                        // Read the values that are currently on the screen
                                        if let ConnectionStatus::Connected = self.connection_status
                                        {
                                            self.modbus_read_current_page().await;
                                        } else {
                                            let _ = self
                                                .sender
                                                .send(Action::Error(String::from(
                                                    "Connect to a server first.",
                                                )))
                                                .await;
                                        }
                                    }
                                    KeyCode::Char('R') => {
                                        self.page_refresh = match self.page_refresh {
                                            true => false,
                                            false => true,
                                        }
                                    }
                                    KeyCode::Char('T') => {
                                        self.tick_refresh = match self.tick_refresh {
                                            true => false,
                                            false => true,
                                        }
                                    }
                                    KeyCode::Char('u') => {
                                        if let ConnectionStatus::Connected = self.connection_status
                                        {
                                            self.table_revert_current_cell();
                                        }
                                    }
                                    KeyCode::Char('g') => {
                                        self.app_mode = AppMode::Popup(PopupType::Goto);
                                    }
                                    KeyCode::Enter => {
                                        if let ConnectionStatus::Connected = self.connection_status
                                        {
                                            self.modbus_apply_queued().await;
                                        } else {
                                            let _ = self
                                                .sender
                                                .send(Action::Error(String::from(
                                                    "Connect to a server first.",
                                                )))
                                                .await;
                                        }
                                    }
                                    KeyCode::Char(' ') => {
                                        if let ConnectionStatus::Connected = self.connection_status
                                        {
                                            match self.selected_top_tab {
                                                SelectedTopTab::Coils => {
                                                    self.table_toggle_current_cell()
                                                }
                                                SelectedTopTab::HoldingRegisters => {
                                                    self.app_mode = AppMode::Popup(PopupType::Edit)
                                                }
                                                _ => {}
                                            }
                                        } else {
                                            let _ = self
                                                .sender
                                                .send(Action::Error(String::from(
                                                    "Connect to a server first.",
                                                )))
                                                .await;
                                        }
                                    }
                                    KeyCode::Char('?') => self.app_mode = AppMode::Help,
                                    _ => {}
                                }
                            }
                            CurrentFocus::Bottom => {
                                match key.code {
                                    KeyCode::Esc => self.exit = true,
                                    KeyCode::Tab => self.current_focus = CurrentFocus::Top,
                                    KeyCode::Char('q') => self.previous_bottom_tab(),
                                    KeyCode::Char('e') => self.next_bottom_tab(),
                                    KeyCode::Char('?') => self.app_mode = AppMode::Help,
                                    _ => {}
                                }
                                match self.selected_bottom_tab {
                                    SelectedBottomTab::Connection => match key.code {
                                        KeyCode::Left | KeyCode::Char('a') => {
                                            if let SelectedConnectionButton::Disconnect =
                                                self.selected_connection_button
                                            {
                                                self.selected_connection_button =
                                                    SelectedConnectionButton::NewConnection;
                                            }
                                        }
                                        KeyCode::Right | KeyCode::Char('d') => {
                                            if let SelectedConnectionButton::NewConnection =
                                                self.selected_connection_button
                                            {
                                                self.selected_connection_button =
                                                    SelectedConnectionButton::Disconnect;
                                            }
                                        }
                                        KeyCode::Enter => match self.selected_connection_button {
                                            SelectedConnectionButton::NewConnection => {
                                                self.app_mode =
                                                    AppMode::Popup(PopupType::Connection);
                                            }
                                            SelectedConnectionButton::Disconnect => {
                                                self.sender.send(Action::Disconnect).await?
                                            }
                                        },
                                        _ => {}
                                    },
                                    SelectedBottomTab::Queue => match key.code {
                                        KeyCode::Up => {
                                            self.queue_select_previous_item();
                                        }
                                        KeyCode::Down => {
                                            self.queue_select_next_item();
                                        }
                                        KeyCode::Char('g') => {
                                            if !self.queue_table_data.is_empty() {
                                                self.queue_go_to_cell(
                                                    self.queue_table_data[self.queue_item_index]
                                                        .address,
                                                    self.queue_table_data[self.queue_item_index]
                                                        .table_index,
                                                );
                                            }
                                        }
                                        KeyCode::Char('r') => {
                                            if !self.queue_table_data.is_empty() {
                                                self.queue_revert_item()
                                            }
                                        }
                                        KeyCode::Char('m') => {
                                            if let ConnectionStatus::Connected =
                                                self.connection_status
                                            {
                                                if !self.queue_table_data.is_empty() {
                                                    self.app_mode = AppMode::Popup(
                                                        PopupType::SaveMacro(SaveMacroMode::Main),
                                                    );
                                                } else {
                                                    let _ = self
                                                        .sender
                                                        .send(Action::Error(String::from(
                                                            "Queue some commands first",
                                                        )))
                                                        .await;
                                                }
                                            } else {
                                                let _ = self
                                                    .sender
                                                    .send(Action::Error(String::from(
                                                        "Connect to a server first",
                                                    )))
                                                    .await;
                                            }
                                        }
                                        _ => {}
                                    },
                                }
                            }
                        }
                    }
                    AppMode::Help => match key.code {
                        KeyCode::Esc => self.exit = true,
                        KeyCode::Char('?') => self.app_mode = AppMode::Main,
                        KeyCode::Tab => self.help_menu_page = match self.help_menu_page {
                            0 => 1,
                            _ => 0,
                        },
                        _ => {}
                    },
                    AppMode::Popup(popup) => match popup {
                        PopupType::Connection => match key.code {
                            KeyCode::Esc => self.exit = true,
                            KeyCode::Backspace => match self.connecting_popup_field {
                                ConnectingField::Address => {
                                    if self.address_input_cursor > 0 {
                                        self.address_input.remove(self.address_input_cursor - 1);
                                        self.address_input_cursor =
                                            self.address_input_cursor.saturating_sub(1);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                                ConnectingField::Port => {
                                    if self.port_input_cursor > 0 {
                                        self.port_input.remove(self.port_input_cursor - 1);
                                        self.port_input_cursor =
                                            self.port_input_cursor.saturating_sub(1);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                            },
                            KeyCode::Enter => {
                                if self.address_input.len() < 2 || self.port_input.len() < 2 {
                                    self.beep()?;
                                }

                                let address = (self.address_input.as_str().trim().to_owned()
                                    + ":"
                                    + self.port_input.as_str().trim())
                                .parse::<SocketAddr>();

                                match address {
                                    Ok(addr) => {
                                        self.app_mode = AppMode::Main;

                                        self.address_input = String::from(" ");
                                        self.address_input_cursor = 0;

                                        self.port_input = String::from(" ");
                                        self.port_input_cursor = 0;

                                        self.connecting_popup_field = ConnectingField::Address;

                                        self.sender.send(Action::Connect(addr)).await?;
                                    }
                                    Err(_) => self.beep()?,
                                }
                            }
                            KeyCode::Left => match self.connecting_popup_field {
                                ConnectingField::Address => {
                                    self.address_input_cursor =
                                        self.address_input_cursor.saturating_sub(1)
                                }
                                ConnectingField::Port => {
                                    self.port_input_cursor =
                                        self.port_input_cursor.saturating_sub(1)
                                }
                            },
                            KeyCode::Right => match self.connecting_popup_field {
                                ConnectingField::Address => {
                                    if self.address_input_cursor < self.address_input.len() - 1 {
                                        self.address_input_cursor =
                                            self.address_input_cursor.saturating_add(1);
                                    }
                                }
                                ConnectingField::Port => {
                                    if self.port_input_cursor < self.port_input.len() - 1 {
                                        self.port_input_cursor =
                                            self.port_input_cursor.saturating_add(1);
                                    }
                                }
                            },
                            KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                                self.connecting_popup_field = match self.connecting_popup_field {
                                    ConnectingField::Address => ConnectingField::Port,
                                    ConnectingField::Port => ConnectingField::Address,
                                }
                            }
                            KeyCode::Delete => match self.connecting_popup_field {
                                ConnectingField::Address => {
                                    if self.address_input_cursor < self.address_input.len() - 1 {
                                        self.address_input.remove(self.address_input_cursor);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                                ConnectingField::Port => {
                                    if self.address_input_cursor < self.address_input.len() - 1 {
                                        self.address_input.remove(self.address_input_cursor);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                            },
                            KeyCode::Char(c) => match self.connecting_popup_field {
                                ConnectingField::Address => {
                                    if self.is_address_char(c) {
                                        self.address_input.insert(self.address_input_cursor, c);
                                        self.address_input_cursor =
                                            self.address_input_cursor.saturating_add(1);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                                ConnectingField::Port => {
                                    if c.is_ascii_digit() {
                                        self.port_input.insert(self.port_input_cursor, c);
                                        self.port_input_cursor =
                                            self.port_input_cursor.saturating_add(1);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                            },
                            _ => {}
                        },
                        PopupType::Edit => match key.code {
                            KeyCode::Esc => {
                                self.edit_popup_cursor = 0;
                                self.edit_popup_input = String::new();
                                self.app_mode = AppMode::Main;
                            }
                            KeyCode::Backspace => {
                                if self.edit_popup_cursor > 0 {
                                    self.edit_popup_input.pop();
                                    self.edit_popup_cursor =
                                        self.edit_popup_cursor.saturating_sub(1);
                                } else {
                                    self.beep()?;
                                }
                            }
                            KeyCode::Enter => {
                                if let Ok(new_value) = self.edit_popup_input.parse::<usize>() {
                                    if new_value > 65535 {
                                        self.beep()?;
                                    } else {
                                        self.table_queue_current_cell(new_value as u16);
                                        self.edit_popup_cursor = 0;
                                        self.edit_popup_input = String::new();
                                        self.app_mode = AppMode::Main;
                                    }
                                } else {
                                    self.beep()?;
                                }
                            }
                            KeyCode::Char(c) => {
                                if c.is_ascii_digit() && self.edit_popup_cursor < 5 {
                                    self.edit_popup_input.push(c);
                                    self.edit_popup_cursor =
                                        self.edit_popup_cursor.saturating_add(1);
                                } else {
                                    self.beep()?;
                                }
                            }
                            _ => {}
                        },
                        PopupType::Error(_) => {
                            if key.code == KeyCode::Enter {
                                self.app_mode = AppMode::Main;
                            }
                        }
                        PopupType::Goto => match key.code {
                            KeyCode::Esc => {
                                self.goto_popup_cursor = 0;
                                self.goto_popup_input = String::new();
                                self.app_mode = AppMode::Main;
                            }
                            KeyCode::Backspace => {
                                if self.goto_popup_cursor > 0 {
                                    self.goto_popup_input.pop();
                                    self.goto_popup_cursor =
                                        self.goto_popup_cursor.saturating_sub(1);
                                } else {
                                    self.beep()?;
                                }
                            }
                            KeyCode::Enter => {
                                if let Ok(new_value) = self.goto_popup_input.parse::<usize>() {
                                    if !(1..=65535).contains(&new_value) {
                                        self.beep()?;
                                    } else {
                                        self.table_go_to_cell((new_value - 1) as u16);
                                        self.goto_popup_cursor = 0;
                                        self.goto_popup_input = String::new();
                                        self.app_mode = AppMode::Main;
                                    }
                                } else {
                                    self.beep()?;
                                }
                            }
                            KeyCode::Char(c) => {
                                if c.is_ascii_digit() && self.goto_popup_cursor < 5 {
                                    self.goto_popup_input.push(c);
                                    self.goto_popup_cursor =
                                        self.goto_popup_cursor.saturating_add(1);
                                } else {
                                    self.beep()?;
                                }
                            }
                            _ => {}
                        },
                        PopupType::SaveMacro(save_macro_mode) => match save_macro_mode {
                            SaveMacroMode::Main => match key.code {
                                KeyCode::Esc => {
                                    self.macro_popup_cursor = 0;
                                    self.macro_popup_input = String::new();
                                    self.app_mode = AppMode::Main;
                                }
                                KeyCode::Backspace => {
                                    if self.macro_popup_cursor > 0 {
                                        self.macro_popup_input.pop();
                                        self.macro_popup_cursor =
                                            self.macro_popup_cursor.saturating_sub(1);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                                KeyCode::Enter => {
                                    let magmod_contents = MagModCommandList::new(
                                        self.current_ip_address
                                            .expect("This shouldn't be possible")
                                            .into(),
                                        self.current_port.expect("This shouldn't be possible"),
                                        self.queue_table_data
                                            .iter()
                                            .map(|queue_item| {
                                                (
                                                    queue_item.cell.table_type,
                                                    queue_item.address,
                                                    queue_item.cell.queued_content,
                                                )
                                            })
                                            .collect(),
                                    );
                                    match magmod_contents
                                        .to_file(self.macro_popup_input.clone(), false)
                                        .await
                                    {
                                        Ok(_) => {
                                            self.macro_popup_input = String::new();
                                            self.macro_popup_cursor = 0;
                                            self.app_mode = AppMode::Popup(PopupType::SaveMacro(
                                                SaveMacroMode::FileSaved,
                                            ));
                                        }
                                        Err(err) => {
                                            if let std::io::ErrorKind::AlreadyExists = err.kind() {
                                                self.app_mode =
                                                    AppMode::Popup(PopupType::SaveMacro(
                                                        SaveMacroMode::OverwriteWarning,
                                                    ));
                                            } else {
                                                self.app_mode = AppMode::Main;
                                                let _ = self
                                                    .sender
                                                    .send(Action::Error(err.kind().to_string()))
                                                    .await;
                                            }
                                        }
                                    };
                                }
                                KeyCode::Char(c) => {
                                    if (c.is_alphanumeric() || matches!(c, '_' | '-'))
                                        && self.macro_popup_cursor < 50
                                    {
                                        self.macro_popup_input.push(c);
                                        self.macro_popup_cursor =
                                            self.macro_popup_cursor.saturating_add(1);
                                    } else {
                                        self.beep()?;
                                    }
                                }
                                _ => {}
                            },
                            SaveMacroMode::OverwriteWarning => match key.code {
                                KeyCode::Esc => {
                                    self.macro_popup_cursor = 0;
                                    self.macro_popup_input = String::new();
                                    self.app_mode = AppMode::Main;
                                }
                                KeyCode::Char('y') => {
                                    let magmod_contents = MagModCommandList::new(
                                        self.current_ip_address
                                            .expect("This shouldn't be possible")
                                            .into(),
                                        self.current_port.expect("This shouldn't be possible"),
                                        self.queue_table_data
                                            .iter()
                                            .map(|queue_item| {
                                                (
                                                    queue_item.cell.table_type,
                                                    queue_item.address,
                                                    queue_item.cell.queued_content,
                                                )
                                            })
                                            .collect(),
                                    );
                                    match magmod_contents
                                        .to_file(self.macro_popup_input.clone(), true)
                                        .await
                                    {
                                        Ok(_) => {
                                            self.macro_popup_input = String::new();
                                            self.macro_popup_cursor = 0;
                                            self.app_mode = AppMode::Popup(PopupType::SaveMacro(
                                                SaveMacroMode::FileSaved,
                                            ));
                                        }
                                        Err(err) => {
                                            self.app_mode = AppMode::Main;
                                            let _ = self
                                                .sender
                                                .send(Action::Error(err.kind().to_string()))
                                                .await;
                                        }
                                    };
                                }
                                KeyCode::Char('n') => {
                                    self.app_mode =
                                        AppMode::Popup(PopupType::SaveMacro(SaveMacroMode::Main))
                                }
                                _ => {}
                            },
                            SaveMacroMode::FileSaved => if key.code == KeyCode::Enter {
                                self.app_mode = AppMode::Main;
                            },
                        },
                    },
                }
            }
        }
        Ok(())
    }

    fn apply_modbus_updates(&mut self, commands: Vec<ModbusWriteCommand>) {
        for (table_index, address, content) in commands {
            let table = &mut self.tables[table_index as usize];
            table.set_cell(address, content);
        }
        self.refresh_queue_table();
    }

    fn render(&mut self, frame: &mut Frame) {
        let [header_area, inner_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .areas(frame.area());

        let [top_area, bottom_area] =
            Layout::vertical([Constraint::Length(11), Constraint::Min(0)]).areas(inner_area);

        self.set_colors();

        match self.app_mode.clone() {
            AppMode::Main => {
                self.render_header(frame, header_area);
                self.render_footer(frame, footer_area);
                self.render_top_areas(frame, top_area);
                self.render_bottom_areas(frame, bottom_area);
            }
            AppMode::Help => {
                self.render_help_menu(frame, frame.area());
            }
            AppMode::Popup(popup_type) => {
                self.render_header(frame, header_area);
                self.render_footer(frame, footer_area);
                self.render_top_areas(frame, top_area);
                self.render_bottom_areas(frame, bottom_area);

                match popup_type {
                    PopupType::Connection => self.render_connection_popup(frame, frame.area()),
                    PopupType::Edit => self.render_edit_popup(frame, frame.area()),
                    PopupType::Error(message) => {
                        self.render_error_popup(frame, frame.area(), message)
                    }
                    PopupType::Goto => self.render_goto_popup(frame, frame.area()),
                    PopupType::SaveMacro(save_macro_mode) => {
                        self.render_macro_popup(frame, frame.area(), save_macro_mode)
                    }
                }
            }
        }
    }

    fn render_header(&self, frame: &mut Frame, header_area: Rect) {
        let [title_version_area, _, address_area] = Layout::horizontal([
            Constraint::Length(22),
            Constraint::Fill(1),
            Constraint::Length(32),
        ])
        .areas(header_area);

        let title_version = Line::from(vec![Span::styled(
            format!("Magic ModBus - v{}", env!("CARGO_PKG_VERSION")),
            Style::default(),
        )])
        .left_aligned();

        let selected_tab_index = self.selected_top_tab as usize;
        let table = &self.tables[selected_tab_index];

        let memory_address = match self.selected_top_tab {
            SelectedTopTab::Coils => format!("0x0{:04X}", table.table_address + 1),
            SelectedTopTab::DiscreteInputs => format!("0x1{:04X}", table.table_address + 1),
            SelectedTopTab::InputRegisters => format!("0x3{:04X}", table.table_address + 1),
            SelectedTopTab::HoldingRegisters => format!("0x4{:04X}", table.table_address + 1),
        };

        let ip_section_style = match self.connection_status {
            ConnectionStatus::Connected => self.colors.connection_connected_fg,
            ConnectionStatus::NotConnected => self.colors.connection_not_selected_fg,
        };

        let ip_section_content = match (self.current_ip_address, self.current_port) {
            (Some(address), Some(port)) => format!("{}:{}", address, port),
            _ => String::from("Not Connected!"),
        };

        let ip_cell_address = Line::from(vec![
            Span::styled(ip_section_content, ip_section_style),
            Span::raw(" | "),
            Span::styled(memory_address, Style::default()),
        ])
        .right_aligned();

        frame.render_widget(title_version, title_version_area);
        frame.render_widget(ip_cell_address, address_area);
    }

    fn render_footer(&self, frame: &mut Frame, footer_area: Rect) {
        let lower_footer_text = match self.current_focus {
            CurrentFocus::Top => FOOTER_TEXT[1],
            CurrentFocus::Bottom => match self.selected_bottom_tab {
                SelectedBottomTab::Connection => FOOTER_TEXT[2],
                SelectedBottomTab::Queue => FOOTER_TEXT[3],
            },
        };
        let test_footer = Text::from(vec![
            Line::styled(FOOTER_TEXT[0], Style::default()).centered(),
            Line::styled(lower_footer_text, self.colors.section_selected_fg).centered(),
        ]);

        frame.render_widget(test_footer, footer_area);
    }

    fn render_top_areas(&self, frame: &mut Frame, top_area: Rect) {
        let [tab_area, cell_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(top_area);

        let area_style = match self.current_focus {
            CurrentFocus::Top => self.colors.section_selected_fg,
            CurrentFocus::Bottom => self.colors.section_unselected_fg,
        };

        let titles = SelectedTopTab::iter().map(SelectedTopTab::title);
        let selected_tab_index = self.selected_top_tab as usize;
        let top_tabs = Tabs::new(titles)
            .select(selected_tab_index)
            .padding("", "")
            .style(area_style);

        frame.render_widget(top_tabs, tab_area);
        self.render_table(frame, cell_area);
    }

    fn render_bottom_areas(&mut self, frame: &mut Frame, bottom_area: Rect) {
        let [tab_area, main_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(bottom_area);

        let area_style = match self.current_focus {
            CurrentFocus::Top => self.colors.section_unselected_fg,
            CurrentFocus::Bottom => self.colors.section_selected_fg,
        };

        let titles = SelectedBottomTab::iter().map(SelectedBottomTab::title);
        let selected_tab_index = self.selected_bottom_tab as usize;
        let bottom_tabs = Tabs::new(titles)
            .select(selected_tab_index)
            .padding("", "")
            .style(area_style);

        frame.render_widget(bottom_tabs, tab_area);

        match self.selected_bottom_tab {
            SelectedBottomTab::Connection => self.render_connection_tab(frame, main_area),
            SelectedBottomTab::Queue => self.render_queue_tab(frame, main_area),
        }
    }

    fn render_connection_tab(&self, frame: &mut Frame, area: Rect) {
        let area_style = match self.current_focus {
            CurrentFocus::Top => self.colors.section_unselected_fg,
            CurrentFocus::Bottom => self.colors.section_selected_fg,
        };

        let selected_button_style = Style::from(area_style).add_modifier(Modifier::REVERSED);

        frame.render_widget(Block::bordered().style(area_style), area);

        let trimmed_area = trim_borders(area);

        let [stats_area, buttons_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(trimmed_area);

        let [connect_button_area, disconnect_button_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(buttons_area);

        let address = match self.current_ip_address {
            None => String::from("N\\A"),
            Some(addr) => addr.to_string(),
        };

        let port = match self.current_port {
            None => String::from("N\\A"),
            Some(port) => port.to_string(),
        };

        let connection_stats = Paragraph::new(vec![
            Line::from(format!("Connection Status: {}", self.connection_status)),
            Line::from(format!("Target Address: {}", address)),
            Line::from(format!("Target Port: {}", port)),
        ]);

        let connection_button = Paragraph::new(vec![
            Line::from("New Connection")
                .style(match self.selected_connection_button {
                    SelectedConnectionButton::NewConnection => selected_button_style,
                    SelectedConnectionButton::Disconnect => Style::new(),
                })
                .centered(),
        ])
        .block(Block::bordered());
        let disconnect_button = Paragraph::new(vec![
            Line::from("Disconnect")
                .style(match self.selected_connection_button {
                    SelectedConnectionButton::NewConnection => Style::new(),
                    SelectedConnectionButton::Disconnect => selected_button_style,
                })
                .centered(),
        ])
        .block(Block::bordered());

        frame.render_widget(connection_stats, stats_area);
        frame.render_widget(connection_button, connect_button_area);
        frame.render_widget(disconnect_button, disconnect_button_area);
    }

    fn render_queue_tab(&mut self, frame: &mut Frame, area: Rect) {
        let area_style = match self.current_focus {
            CurrentFocus::Top => self.colors.section_unselected_fg,
            CurrentFocus::Bottom => self.colors.section_selected_fg,
        };

        self.queue_scroll_state = self
            .queue_scroll_state
            .content_length(self.queue_table_data.len());

        self.queue_table_data
            .sort_by_key(|queue_item| queue_item.address);
        self.queue_table_data
            .sort_by_key(|queue_item| queue_item.table_index);

        if !self.queue_table_data.is_empty() {
            let mut rows = vec![];
            for queue_item in self.queue_table_data.iter() {
                rows.push(Row::new(vec![
                    queue_item.cell.table_type.to_string(),
                    format!("0x{:04X}", queue_item.address + 1),
                    queue_item.original_content(),
                    "->".to_string(),
                    queue_item.queued_content(),
                ]));
            }

            let table = Table::new(
                rows,
                [
                    Constraint::Length(17),
                    Constraint::Length(6),
                    Constraint::Length(5),
                    Constraint::Length(2),
                    Constraint::Length(5),
                ],
            )
            .block(Block::bordered().style(area_style))
            .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED));

            frame.render_stateful_widget(table, area, &mut self.queue_table_state);

            if area.height - 2 < self.queue_table_data.len() as u16 {
                frame.render_stateful_widget(
                    Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                    area.inner(Margin {
                        vertical: 1,
                        horizontal: 1,
                    }),
                    &mut self.queue_scroll_state,
                );
            }
        } else {
            frame.render_widget(
                Paragraph::new("No Queued Commands").block(Block::bordered().style(area_style)),
                area,
            )
        }
    }

    fn render_help_menu(&self, frame: &mut Frame, area: Rect) {
        let help_menu_block = Block::bordered()
            .title(format!("Magic ModBus - Help Menu (Page {}/2)", self.help_menu_page + 1))
            .title_alignment(Alignment::Center)
            .style(self.colors.section_selected_fg);
        frame.render_widget(help_menu_block, area);

        let trimmed_area = trim_borders(area);

        let [general_area, table_area] = Layout::vertical([
            Constraint::Length(6),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
            .areas(trimmed_area);

        let [connection_area, queue_area, _, help_hint_area] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
            .areas(trimmed_area);

        // General Controls Section
        let general_help = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("ESC", Style::default().bold()),
                Span::raw(" - Quit Application"),
            ]),
            Line::from(vec![
                Span::styled("TAB", Style::default().bold()),
                Span::raw(" - Switch Focus (Top/Bottom panels)"),
            ]),
            Line::from(vec![
                Span::styled("?", Style::default().bold()),
                Span::raw(" - Toggle Help Menu"),
            ]),
            Line::from(vec![
                Span::styled("Q/E", Style::default().bold()),
                Span::raw(" - Previous/Next Tab"),
            ]),
        ])
        .block(
            Block::new()
                .borders(Borders::BOTTOM)
                .title("General Controls"),
        );

        // Table Navigation Section
        let table_help = Paragraph::new(vec![
            Line::from("Navigation (when focused on top panel):"),
            Line::from(vec![
                Span::styled("W/A/S/D", Style::default().bold()),
                Span::raw(" or "),
                Span::styled("Arrow Keys", Style::default().bold()),
                Span::raw(" - Move cursor"),
            ]),
            Line::from(vec![
                Span::styled("Shift+W/S", Style::default().bold()),
                Span::raw(" or "),
                Span::styled("Shift+↑/↓", Style::default().bold()),
                Span::raw(" - Page up/down"),
            ]),
            Line::from(vec![
                Span::styled("G", Style::default().bold()),
                Span::raw(" - Go to address (1-65535)"),
            ]),
            Line::raw(""),
            Line::from("Data Operations:"),
            Line::from(vec![
                Span::styled("SPACE", Style::default().bold()),
                Span::raw(" - Toggle Coils / Edit Holding Registers"),
            ]),
            Line::from(vec![
                Span::styled("ENTER", Style::default().bold()),
                Span::raw(" - Apply all queued changes"),
            ]),
            Line::from(vec![
                Span::styled("U", Style::default().bold()),
                Span::raw(" - Revert current cell"),
            ]),
            Line::from(vec![
                Span::styled("R", Style::default().bold()),
                Span::raw(" - Read current page"),
            ]),
            Line::from(vec![
                Span::styled("Shift+R", Style::default().bold()),
                Span::raw(" - Toggle auto page refresh"),
            ]),
            Line::from(vec![
                Span::styled("Shift+T", Style::default().bold()),
                Span::raw(" - Toggle auto tick refresh"),
            ]),
        ])
        .block(
            Block::new()
                // .borders(Borders::BOTTOM)
                .title("Table Navigation & Data"),
        )
        .wrap(Wrap { trim: true });

        // Connection Tab Section
        let connection_help = Paragraph::new(vec![
            Line::from("When focused on Connection tab (bottom panel):"),
            Line::from(vec![
                Span::styled("A/D", Style::default().bold()),
                Span::raw(" or "),
                Span::styled("←/→", Style::default().bold()),
                Span::raw(" - Select connection button"),
            ]),
            Line::from(vec![
                Span::styled("ENTER", Style::default().bold()),
                Span::raw(" - New Connection or Disconnect"),
            ]),
            Line::raw(""),
            Line::from("In Connection Popup:"),
            Line::from("• Enter IP address and port"),
            Line::from("• Use UP/DOWN/TAB to switch fields"),
        ])
        .block(
            Block::new()
                .borders(Borders::BOTTOM)
                .title("Connection Controls"),
        );

        // Queue Tab Section
        let queue_help = Paragraph::new(vec![
            Line::from("When focused on Queue tab (bottom panel):"),
            Line::from(vec![
                Span::styled("↑/↓", Style::default().bold()),
                Span::raw(" - Navigate queue items"),
            ]),
            Line::from(vec![
                Span::styled("G", Style::default().bold()),
                Span::raw(" - Go to selected queue item's address"),
            ]),
            Line::from(vec![
                Span::styled("R", Style::default().bold()),
                Span::raw(" - Revert selected queue item"),
            ]),
            Line::from(vec![
                Span::styled("M", Style::default().bold()),
                Span::raw(" - Save queue as macro file"),
            ]),
        ])
        .block(Block::new().title("Queue Controls"));

        let help_hint = Paragraph::new("Press 'Tab' to change pages").centered();

        match self.help_menu_page {
            0 => {
                frame.render_widget(general_help, general_area);
                frame.render_widget(table_help, table_area);
            }
            _ => {
                frame.render_widget(connection_help, connection_area);
                frame.render_widget(queue_help, queue_area);
            }
        }
        frame.render_widget(help_hint, help_hint_area);
    }

    fn render_table(&self, frame: &mut Frame, table_area: Rect) {
        let selected_tab_index = self.selected_top_tab as usize;
        let mut table = self.tables[selected_tab_index].to_owned();
        let area_style = match self.current_focus {
            CurrentFocus::Top => self.colors.section_selected_fg,
            CurrentFocus::Bottom => self.colors.section_unselected_fg,
        };

        let selected_cell_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(match self.current_focus {
                CurrentFocus::Top => self.colors.table_selected_cell_fg,
                CurrentFocus::Bottom => self.colors.table_unselected_cell_fg,
            });

        let block = Block::bordered().style(area_style);

        let (_row_height, column_length, max_rows, max_cols) = self.get_table_stats(table_area);
        table.table_rows = max_rows;
        table.table_cols = max_cols;

        let start_index = table.page_offset * table.page_size();
        let end_index = usize::min(start_index + table.page_size(), (u16::MAX - 1) as usize);

        let visible_data = table.get_visible_data(start_index as u16, end_index as u16);

        let table_rows = visible_data
            .chunks(table.table_cols)
            .enumerate()
            .map(|(i, row_chunk)| {
                let row = row_chunk
                    .iter()
                    .enumerate()
                    .map(|(j, cell)| {
                        let row_parity = i % 2;
                        let cell_parity = j % 2;
                        let cell_content = match self.selected_top_tab {
                            SelectedTopTab::Coils | SelectedTopTab::DiscreteInputs => {
                                Line::raw(format!(
                                    "{}",
                                    match &cell.state {
                                        CellState::Normal => cell.original_content.to_u16(),
                                        CellState::Queued => cell.queued_content.to_u16(),
                                    }
                                ))
                                .centered()
                                .style(Style::new().fg(Color::White))
                            }
                            SelectedTopTab::InputRegisters | SelectedTopTab::HoldingRegisters => {
                                Line::raw(format!(
                                    "{:05}",
                                    match &cell.state {
                                        CellState::Normal => cell.original_content.to_u16(),
                                        CellState::Queued => cell.queued_content.to_u16(),
                                    }
                                ))
                                .centered()
                                .style(Style::new().fg(Color::White))
                            }
                        };

                        let color = match (row_parity + cell_parity) % 2 {
                            0 => match self.current_focus {
                                CurrentFocus::Top => self.colors.table_normal_cell_bg,
                                CurrentFocus::Bottom => self.colors.table_unselected_normal_cell_bg,
                            },
                            _ => match self.current_focus {
                                CurrentFocus::Top => self.colors.table_alt_cell_bg,
                                CurrentFocus::Bottom => self.colors.table_unselected_alt_cell_bg,
                            },
                        };

                        match cell.state {
                            CellState::Normal => {
                                Cell::from(cell_content).style(Style::new().bg(color))
                            }
                            CellState::Queued => Cell::from(cell_content)
                                .style(Style::new().bg(color).bold().underlined()),
                        }
                    })
                    .collect::<Vec<Cell>>();
                Row::new(row)
            })
            .collect::<Vec<Row>>();

        let widths = vec![Constraint::Length(column_length as u16); table.table_cols];

        let cell_table = Table::new(table_rows, widths)
            .block(block)
            .column_spacing(0)
            .cell_highlight_style(selected_cell_style);
        frame.render_stateful_widget(cell_table, table_area, &mut table.table_state);
    }

    fn render_connection_popup(&self, frame: &mut Frame, popup_area: Rect) {
        let area_style = match self.current_focus {
            CurrentFocus::Top => self.colors.section_unselected_fg,
            CurrentFocus::Bottom => self.colors.section_selected_fg,
        };

        let area = centered_rect(CONNECTION_POPUP_TEXT.len() as u16 + 2, 6, popup_area);
        frame.render_widget(Clear, area);
        frame.render_widget(Block::bordered().style(area_style), area);

        let (address_cursor_style, address_field_style) = match self.connecting_popup_field {
            ConnectingField::Address => (
                Style::from(area_style).add_modifier(Modifier::REVERSED),
                Style::from(area_style).add_modifier(Modifier::UNDERLINED),
            ),
            ConnectingField::Port => (Style::from(area_style), Style::from(area_style)),
        };

        let (port_cursor_style, port_field_style) = match self.connecting_popup_field {
            ConnectingField::Address => (Style::from(area_style), Style::from(area_style)),
            ConnectingField::Port => (
                Style::from(area_style).add_modifier(Modifier::REVERSED),
                Style::from(area_style).add_modifier(Modifier::UNDERLINED),
            ),
        };

        // Refit the area to account for the borders
        let trimmed_area = trim_borders(area);
        let address_line = Line::from(vec![
            Span::styled("Address:", address_field_style),
            Span::raw(" "),
            Span::from(&self.address_input[..self.address_input_cursor]),
            Span::styled(
                format!(
                    "{}",
                    &self
                        .address_input
                        .chars()
                        .nth(self.address_input_cursor)
                        .unwrap()
                ),
                address_cursor_style,
            ),
            Span::from(&self.address_input[(self.address_input_cursor + 1)..]),
        ]);
        let port_line = Line::from(vec![
            Span::raw("   "),
            Span::styled("Port:", port_field_style),
            Span::raw(" "),
            Span::from(&self.port_input[..self.port_input_cursor]),
            Span::styled(
                format!(
                    "{}",
                    &self.port_input.chars().nth(self.port_input_cursor).unwrap()
                ),
                port_cursor_style,
            ),
            Span::from(&self.port_input[(self.port_input_cursor + 1)..]),
        ]);

        let popup_content = Paragraph::new(vec![
            Line::from(CONNECTION_POPUP_TEXT),
            Line::from("-".repeat(CONNECTION_POPUP_TEXT.len())),
            address_line,
            port_line,
        ])
        .style(area_style);

        frame.render_widget(popup_content, trimmed_area);
    }

    fn render_edit_popup(&self, frame: &mut Frame, popup_area: Rect) {
        let text_style = Style::new()
            .bg(self.colors.table_normal_cell_bg)
            .fg(Color::White);
        let area = centered_rect(23, 4, popup_area);
        frame.render_widget(Clear, area);

        let popup_content = Paragraph::new(vec![
            Line::raw(" Set Value (0-65535) "),
            Line::from(vec![
                Span::styled(&self.edit_popup_input[..self.edit_popup_cursor], text_style),
                Span::styled(" ".repeat(5 - self.edit_popup_cursor), text_style),
            ])
            .centered(),
        ])
        .block(Block::bordered())
        .style(Style::new().fg(self.colors.section_selected_fg));
        frame.render_widget(popup_content, area);
    }

    fn render_error_popup(&self, frame: &mut Frame, popup_area: Rect, message: String) {
        let area = centered_rect((message.len() + 4) as u16, 5, popup_area);
        frame.render_widget(Clear, area);

        let popup_content = Paragraph::new(vec![
            Line::styled("Error", Style::new())
                .centered()
                .bold()
                .underlined(),
            Line::from(vec![
                Span::raw(" "),
                Span::styled(message, Style::new().fg(Color::White)),
                Span::raw(" "),
            ]),
            Line::raw("Press Enter To Close").centered(),
        ])
        .block(Block::bordered())
        .style(Style::new().fg(self.colors.section_selected_fg));
        frame.render_widget(popup_content, area);
    }

    fn render_goto_popup(&self, frame: &mut Frame, popup_area: Rect) {
        let text_style = Style::new()
            .bg(self.colors.table_normal_cell_bg)
            .fg(Color::White);
        let area = centered_rect(32, 4, popup_area);
        frame.render_widget(Clear, area);

        let popup_content = Paragraph::new(vec![
            Line::raw(" Seek to an address (1-65535) "),
            Line::from(vec![
                Span::styled(&self.edit_popup_input[..self.edit_popup_cursor], text_style),
                Span::styled(" ".repeat(5 - self.edit_popup_cursor), text_style),
            ])
            .centered(),
        ])
        .block(Block::bordered())
        .style(Style::new().fg(self.colors.section_selected_fg));
        frame.render_widget(popup_content, area);
    }

    fn render_macro_popup(&self, frame: &mut Frame, popup_area: Rect, popup_mode: SaveMacroMode) {
        let text_style = Style::new()
            .bg(self.colors.table_normal_cell_bg)
            .fg(Color::White);
        let area;
        let popup_content;

        let main_message = String::from(" Enter a filename below (extension not required). ");
        let overwrite_warning_message =
            String::from(" Warning - File already exists! Overwrite? (Y/N) ");
        let file_saved_message = String::from(" .magmod file saved to current directory. ");
        match popup_mode {
            SaveMacroMode::Main => {
                area = centered_rect((main_message.len() + 2) as u16, 4, popup_area);
                frame.render_widget(Clear, area);

                popup_content = Paragraph::new(vec![
                    Line::from(main_message.clone()),
                    Line::from(vec![
                        Span::styled(
                            self.macro_popup_input
                                .chars()
                                .rev()
                                .take(main_message.len() + 2)
                                .collect::<Vec<char>>()
                                .into_iter()
                                .rev()
                                .collect::<String>(),
                            text_style,
                        ),
                        Span::styled(
                            " ".repeat(
                                (main_message.len() + 2).saturating_sub(self.macro_popup_cursor),
                            ),
                            text_style,
                        ),
                    ]),
                ])
                .block(Block::bordered())
                .style(Style::new().fg(self.colors.section_selected_fg));
            }
            SaveMacroMode::OverwriteWarning => {
                area = centered_rect((overwrite_warning_message.len() + 2) as u16, 4, popup_area);
                frame.render_widget(Clear, area);

                popup_content = Paragraph::new(vec![
                    Line::styled(
                        "Warning",
                        Style::new()
                            .fg(self.colors.section_selected_fg)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    )
                    .centered(),
                    Line::from(overwrite_warning_message),
                ])
                .block(Block::bordered())
                .style(Style::new().fg(self.colors.section_selected_fg));
            }
            SaveMacroMode::FileSaved => {
                area = centered_rect((file_saved_message.len() + 2) as u16, 3, popup_area);
                frame.render_widget(Clear, area);

                popup_content = Paragraph::new(vec![Line::from(file_saved_message)])
                    .block(Block::bordered())
                    .style(Style::new().fg(self.colors.section_selected_fg));
            }
        }
        frame.render_widget(popup_content, area);
    }

    async fn table_page_up(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.page_up().await;
    }

    async fn table_move_up(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.move_up().await;
    }

    async fn table_page_down(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.page_down().await;
    }

    async fn table_move_down(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.move_down().await;
    }

    fn table_move_left(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.move_left();
    }

    fn table_move_right(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.move_right();
    }

    fn table_go_to_cell(&mut self, cell_address: u16) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.go_to_cell(cell_address);
    }

    fn queue_go_to_cell(&mut self, cell_address: u16, table_index: usize) {
        let table = &mut self.tables[table_index];
        self.selected_top_tab = SelectedTopTab::iter().nth(table_index).unwrap();
        table.go_to_cell(cell_address)
    }

    async fn modbus_apply_queued(&mut self) {
        let commands = self.table_get_queued_commands();
        let _ = self
            .sender
            .send(Action::ToModbus(ModbusCommandQueue::Write(commands)))
            .await;
    }

    fn table_apply_queued_cells(&mut self) {
        for table in &mut self.tables {
            let queued_keys: Vec<u16> = table
                .data
                .iter()
                .filter_map(|(key, value)| {
                    if let CellState::Queued = value.state {
                        Some(*key)
                    } else {
                        None
                    }
                })
                .collect();

            for key in queued_keys {
                if let Some(cell) = table.data.get_mut(&key) {
                    cell.apply();
                }
            }
        }
        self.refresh_queue_table();
    }

    fn refresh_queue_table(&mut self) {
        self.queue_table_data = vec![];
        for table in &self.tables {
            let mut table_queued = table.get_queue_items();
            self.queue_table_data.append(&mut table_queued);
        }
        if self.queue_item_index >= self.queue_table_data.len() && !self.queue_table_data.is_empty()
        {
            self.queue_item_index = self.queue_table_data.len() - 1;
        }
    }

    fn queue_select_next_item(&mut self) {
        self.queue_item_index = match self.queue_table_state.selected() {
            Some(i) => {
                if i >= self.queue_table_data.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.queue_table_state.select(Some(self.queue_item_index));
        self.queue_scroll_state = self.queue_scroll_state.position(self.queue_item_index);
    }

    fn queue_select_previous_item(&mut self) {
        self.queue_item_index = match self.queue_table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.queue_table_data.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };

        self.queue_table_state.select(Some(self.queue_item_index));
        self.queue_scroll_state = self.queue_scroll_state.position(self.queue_item_index);
    }

    fn queue_revert_item(&mut self) {
        let item_address = self.queue_table_data[self.queue_item_index].address;
        let table_index = self.queue_table_data[self.queue_item_index].table_index;

        let table = &mut self.tables[table_index];

        if let Some(item) = table.data.get_mut(&item_address) {
            item.revert();
        }

        self.refresh_queue_table();
    }

    fn table_get_queued_commands(&self) -> Vec<ModbusWriteCommand> {
        let mut commands: Vec<ModbusWriteCommand> = vec![];
        for table in &self.tables {
            let table_commands: Vec<ModbusWriteCommand> = table
                .get_queue_items()
                .into_iter()
                .map(|queue_item| {
                    (
                        table.table_type,
                        queue_item.address,
                        queue_item.cell.queued_content,
                    )
                })
                .collect();
            commands.extend(table_commands);
        }
        commands
    }

    fn table_queue_current_cell(&mut self, new_value: u16) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        match table.table_type {
            SelectedTopTab::Coils | SelectedTopTab::DiscreteInputs => {
                match new_value {
                    0 => table.queue_current_cell(CellType::Coil(false)),
                    _ => table.queue_current_cell(CellType::Coil(true)),
                };
            }
            SelectedTopTab::InputRegisters | SelectedTopTab::HoldingRegisters => {
                table.queue_current_cell(CellType::Word(new_value))
            }
        }
        self.refresh_queue_table();
    }

    fn table_revert_current_cell(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.revert_current_cell();
        self.refresh_queue_table();
    }

    fn table_toggle_current_cell(&mut self) {
        let table = &mut self.tables[self.selected_top_tab as usize];
        table.toggle_current_coil();
        self.refresh_queue_table();
    }

    async fn modbus_read_current_page(&mut self) {
        let table = &self.tables[self.selected_top_tab as usize];
        let amount = (table.table_rows * table.table_cols) as u16;

        if let ConnectionStatus::Connected = self.connection_status {
            let command: Vec<ModbusReadCommand> = vec![(
                self.selected_top_tab,
                table.table_address / amount * amount,
                amount,
            )];
            let _ = self
                .sender
                .send(Action::ToModbus(ModbusCommandQueue::Read(command)))
                .await;
        }
    }

    fn next_top_tab(&mut self) {
        self.selected_top_tab = self.selected_top_tab.next();
    }

    fn previous_top_tab(&mut self) {
        self.selected_top_tab = self.selected_top_tab.previous();
    }

    fn next_bottom_tab(&mut self) {
        self.selected_bottom_tab = self.selected_bottom_tab.next();
    }

    fn previous_bottom_tab(&mut self) {
        self.selected_bottom_tab = self.selected_bottom_tab.previous();
    }

    fn get_table_stats(&self, area: Rect) -> (usize, usize, usize, usize) {
        let row_height: usize = 1;
        let column_length: usize = match self.selected_top_tab {
            SelectedTopTab::Coils | SelectedTopTab::DiscreteInputs => 3,
            SelectedTopTab::InputRegisters | SelectedTopTab::HoldingRegisters => 7,
        };
        let max_rows = (area.height as usize).saturating_sub(2) / row_height;
        let max_cols = match self.selected_top_tab {
            SelectedTopTab::Coils | SelectedTopTab::DiscreteInputs => 16,
            SelectedTopTab::InputRegisters | SelectedTopTab::HoldingRegisters => 8,
        };

        (row_height, column_length, max_rows, max_cols)
    }

    fn set_colors(&mut self) {
        self.colors = AppColors::new(&PALETTES[self.selected_top_tab as usize]);
    }

    fn beep(&self) -> Result<()> {
        print!("\x07");
        std::io::stdout().flush()?;
        Ok(())
    }

    fn is_address_char(&self, c: char) -> bool {
        matches!(c, 'A'..='F' | 'a'..='f' | '0'..='9' | '.' | ':' | '[' | ']' | '%')
    }
}
