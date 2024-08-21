use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter, WriteType};
use btleplug::platform::Manager;
use std::error::Error;
use std::time::Duration;
use tokio::time;
use uuid::Uuid;

const TX_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x0000ae01_0000_1000_8000_00805f9b34fb);

#[derive(Debug)]
pub struct Device {
    peripheral: btleplug::platform::Peripheral,
    supports_compression: bool,
    tx_buffer: Vec<u8>,
    tx_characteristic: btleplug::api::Characteristic,
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
                .start_scan(ScanFilter::default())
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

                            println!("Connected to {}", name);
                            println!("Discovering services...");
                            peripheral.discover_services().await?;

                            let supports_compression = name != "MX10";

                            let characteristics = peripheral.characteristics();

                            println!("Found {} characteristics", characteristics.len());

                            let tx_characteristic = characteristics
                                .iter()
                                .find(|c| c.uuid == TX_CHARACTERISTIC_UUID)
                                .expect("Could not find TX characteristic");

                            device_result = Ok(Device {
                                peripheral,
                                tx_buffer: Vec::new(),
                                supports_compression,
                                tx_characteristic: tx_characteristic.clone(),
                            });
                            break;
                        }
                    }
                }
            }
        }

        println!("Scan complete");
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
        let chunks = self.tx_buffer.chunks(20);

        let write_type = if chunks.len() > 1 {
            WriteType::WithoutResponse
        } else {
            WriteType::WithResponse
        };

        for chunk in chunks {
            self.peripheral
                .write(&self.tx_characteristic, chunk, write_type)
                .await
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(())
    }

    pub async fn destroy(self) {
        self.peripheral.disconnect().await.unwrap();
    }

    pub fn supports_compression(&self) -> bool {
        self.supports_compression
    }
}
