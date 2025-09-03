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
    enums::{CellType, SelectedTopTab},
    utils::{BufReader, ModbusWriteCommand},
};
use inquire::Text;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use tokio_modbus::prelude::*;

#[derive(Debug, PartialEq)]
pub struct MagModCommandList {
    ip_addr: IpAddr,
    port: u16,
    command_count: u32,
    commands: Vec<ModbusWriteCommand>,
}

impl MagModCommandList {
    pub fn new(ip_addr: IpAddr, port: u16, commands: Vec<ModbusWriteCommand>) -> Self {
        Self {
            ip_addr,
            port,
            command_count: commands.len() as u32,
            commands,
        }
    }

    pub async fn to_file(&self, mut filename: String, force: bool) -> std::io::Result<()> {
        let mut bytes = vec![];
        let mut path_buf = std::env::current_dir()?;
        filename = filename.trim().to_string();
        filename.push_str(".magmod");
        path_buf.push(filename);

        let mut file = match force {
            true => File::create(&path_buf).await?,
            false => File::create_new(&path_buf).await?,
        };

        // File extension
        bytes.extend_from_slice(b"MAGMOD");

        // IP Address
        bytes.extend_from_slice(&match self.ip_addr {
            IpAddr::V4(addr) => {
                let mut ip_bytes = vec![4];
                ip_bytes.extend_from_slice(&addr.octets());
                ip_bytes
            }
            IpAddr::V6(addr) => {
                let mut ip_bytes = vec![6];
                ip_bytes.extend_from_slice(&addr.octets());
                ip_bytes
            }
        });

        // Port
        bytes.extend(self.port.to_be_bytes());

        // Command count
        bytes.extend(self.command_count.to_be_bytes());

        for (tab, address, content) in self.commands.iter() {
            match (tab, content) {
                (SelectedTopTab::Coils, CellType::Coil(content)) => {
                    bytes.extend_from_slice(&[5u8]); // Function code 0x05 - Write single coil
                    bytes.extend_from_slice(&address.to_be_bytes());
                    match *content {
                        true => bytes.extend_from_slice(&[0xff, 0x00]),
                        false => bytes.extend_from_slice(&[0x00, 0x00]),
                    }
                }
                (SelectedTopTab::HoldingRegisters, CellType::Word(content)) => {
                    bytes.extend_from_slice(&[6u8]); // Function code 0x06 - Write single register
                    bytes.extend_from_slice(&address.to_be_bytes());
                    bytes.extend_from_slice(&content.to_be_bytes());
                }
                _ => {}
            }
        }

        file.write_all(&bytes).await?;

        Ok(())
    }

    pub async fn from_file<P: AsRef<Path>>(filename: P) -> std::io::Result<Self> {
        let file = fs::read(filename).await?;
        let mut reader = BufReader::new(&file);
        let identifier = reader.read_exact(6).await?;

        if identifier != b"MAGMOD" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Bad header",
            ));
        }

        let ip_protocol = reader.read_u8().await?;
        let ip_addr = match ip_protocol {
            4 => IpAddr::V4(Ipv4Addr::from(
                <[u8; 4]>::try_from(reader.read_exact(4).await?).unwrap(),
            )),
            6 => IpAddr::V6(Ipv6Addr::from(
                <[u8; 16]>::try_from(reader.read_exact(16).await?).unwrap(),
            )),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "File is not a .magmod file.",
                ));
            }
        };

        let port = reader.read_u16().await?;

        let command_count = reader.read_u32().await?;

        let mut commands = Vec::with_capacity(command_count as usize);
        for _ in 0..command_count {
            let table = match reader.read_u8().await? {
                5 => SelectedTopTab::Coils,
                6 => SelectedTopTab::HoldingRegisters,
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Unsupported command.",
                    ));
                }
            };
            let address = reader.read_u16().await?;
            let output_value = reader.read_u16().await?;

            let cell_content = match table {
                SelectedTopTab::Coils => match output_value {
                    0x0000 => CellType::Coil(false),
                    0xff00 => CellType::Coil(true),
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Invalid command.",
                        ));
                    }
                },
                SelectedTopTab::HoldingRegisters => CellType::Word(output_value),
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Unsupported command.",
                    ));
                }
            };
            commands.push((table, address, cell_content));
        }

        Ok(Self {
            ip_addr,
            port,
            command_count,
            commands,
        })
    }

    // Independent of TUI
    pub async fn run_macro(
        &mut self,
        confirm: bool,
        check_connection: bool,
        dry_run: bool,
    ) -> color_eyre::Result<()> {
        if confirm {
            self.ip_addr = Text::new("Confirm Target IP Address")
                .with_default(&self.ip_addr.to_string())
                .prompt()?
                .parse()?;
            self.port = Text::new("Confirm Target Port (1-65535)")
                .with_default(&self.port.to_string())
                .prompt()?
                .parse()?;
        }

        let socket_addr = SocketAddr::new(self.ip_addr, self.port);
        match (check_connection, dry_run) {
            (true, false) => {
                // Check connection only
                println!("Checking connection to {socket_addr}...");
                let mut context = tcp::connect(socket_addr).await?;
                println!("Connection successful.");
                context.disconnect().await?;
            }
            (false, true) => {
                // Dry Run
                println!("[DRY RUN] Connecting to {socket_addr}...");
                println!("[DRY RUN] Connection established. Beginning command-flow...");

                for command in self.commands.iter() {
                    let (address_space, addr, content) = command;
                    match (address_space, content) {
                        (SelectedTopTab::Coils, CellType::Coil(content)) => {
                            println!("[DRY RUN]  Setting Coil 0x0{:04X} to {content}", addr + 1);
                        }
                        (SelectedTopTab::HoldingRegisters, CellType::Word(content)) => {
                            println!(
                                "[DRY RUN]  Setting Register 0x4{:04X} to {content}",
                                addr + 1
                            );
                        }
                        _ => {}
                    }
                }

                println!("[DRY RUN] Command-flow completed. Disconnecting from client...");
            }
            (false, false) => {
                // Normal Run
                println!("Connecting to {socket_addr}...");
                let mut context = tcp::connect(socket_addr).await?;
                println!("Connection established. Beginning command-flow...");

                for command in self.commands.iter() {
                    let (address_space, addr, content) = command;
                    match (address_space, content) {
                        (SelectedTopTab::Coils, CellType::Coil(content)) => {
                            println!("  Setting Coil 0x0{:04X} to {content}", addr + 1);
                            context.write_single_coil(*addr, *content).await??;
                        }
                        (SelectedTopTab::HoldingRegisters, CellType::Word(content)) => {
                            println!("  Setting Register 0x4{:04X} to {content}", addr + 1);
                            context.write_single_register(*addr, *content).await??;
                        }
                        _ => {}
                    }
                }

                println!("Command-flow completed. Disconnecting from client...");
                context.disconnect().await?;
            }
            (_, _) => {}
        }
        Ok(())
    }
}
