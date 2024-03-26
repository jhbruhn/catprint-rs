use btleplug::api::{Central, Manager as _, Peripheral};
use btleplug::platform::Manager;
use std::collections::VecDeque;
use std::error::Error;
use std::time::Duration;
use tokio::time;
use uuid::Uuid;

const TX_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x0000ae01_0000_1000_8000_00805f9b34fb);

#[derive(Debug)]
pub struct Device {
    peripheral: btleplug::platform::Peripheral,
    supports_compression: bool,
    tx_buffer: VecDeque<u8>,
}

impl Device {
    pub async fn find(manager: &mut Manager, name: &str) -> Result<Device, Box<dyn Error>> {
        let mut device_result = Err("Could not connect to device".into());
        let adapter_list = manager.adapters().await?;
        if adapter_list.is_empty() {
            device_result = Err("No Bluetooth adapters found".into());
        }

        for adapter in adapter_list.iter() {
            println!("Starting scan...");
            adapter
                .start_scan()
                .await
                .expect("Can't scan BLE adapter for connected devices...");
            time::sleep(Duration::from_secs(2)).await;

            let peripherals = adapter.peripherals().await?;
            for peripheral in peripherals {
                let properties = peripheral.properties().await?;

                if let Some(properties) = properties {
                    if let Some(local_name) = properties.local_name {
                        if local_name.contains(name) {
                            if !peripheral.is_connected().await? {
                                peripheral.connect().await?;
                            }
                            let _ = peripheral.discover_characteristics().await?;

                            let supports_compression = match name {
                                "MX10" => false,
                                _ => true,
                            };

                            device_result = Ok(Device {
                                peripheral,
                                tx_buffer: VecDeque::new(),
                                supports_compression,
                            });
                            break;
                        }
                    }
                }
            }
        }

        device_result
    }

    pub fn queue_command(&mut self, command: crate::protocol::Command) {
        self.tx_buffer.extend(&command.to_bytes())
    }

    pub fn queue_commands(&mut self, commands: &[crate::protocol::Command]) {
        for command in commands {
            self.queue_command(*command)
        }
    }

    pub async fn flush(&mut self) -> Result<(), Box<dyn Error>> {
        const MTU_SIZE: usize = 20;

        while !self.tx_buffer.is_empty() {
            let mut buf = Vec::with_capacity(MTU_SIZE);
            for _ in 0..MTU_SIZE {
                if let Some(byte) = self.tx_buffer.pop_front() {
                    buf.push(byte);
                } else {
                    break;
                }

                // this could be nicer i guess
            }

            self.tx(&buf).await?;
        }

        Ok(())
    }

    async fn tx(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let characteristics = self.peripheral.characteristics();

        let tx_characteristic = characteristics
            .iter()
            .filter(|c| c.uuid == TX_CHARACTERISTIC_UUID)
            .next()
            .unwrap();

        self.peripheral
            .write(
                &tx_characteristic,
                data,
                btleplug::api::WriteType::WithoutResponse,
            )
            .await?;

        Ok(())
    }

    pub async fn destroy(self) {
        self.peripheral.disconnect().await.unwrap();
    }

    pub fn supports_compression(&self) -> bool {
        self.supports_compression
    }
}
