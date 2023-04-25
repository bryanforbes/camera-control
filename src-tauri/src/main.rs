// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use pelcodrs::{Direction, Message, MessageBuilder, PelcoDPort, Speed};
use serialport::{DataBits, SerialPort, StopBits};
use tauri::Manager;

struct PortState {
    port: Mutex<Option<PelcoDPort<Box<dyn SerialPort>>>>,
}

impl PortState {
    fn send_message(&self, message: Message) -> () {
        self.port
            .lock()
            .unwrap()
            .as_mut()
            .expect("The port is not set")
            .send_message(message)
            .expect("Something went wrong sending message")
    }

    fn set_port(&self, path: Option<&str>) -> () {
        println!("{:?}", path);
        // Drop the previous port implicitly before setting a new one
        *self.port.lock().unwrap() = None;
        *self.port.lock().unwrap() = match path {
            Some(path) => Some(PelcoDPort::new(
                serialport::new(path, 9000)
                    .stop_bits(StopBits::One)
                    .data_bits(DataBits::Eight)
                    .open()
                    .expect("Poop"),
            )),
            _ => None,
        }
    }
}

#[tauri::command]
fn restore_preset(port_state: tauri::State<PortState>, preset: u8) -> () {
    port_state.send_message(Message::go_to_preset(1, preset).unwrap())
}

#[tauri::command]
fn move_camera(port_state: tauri::State<PortState>, direction: &str) -> () {
    println!("Direction: {}", direction);
    port_state.send_message(
        MessageBuilder::new(1)
            .pan(Speed::Range(0.01))
            .tilt(Speed::Range(0.01))
            .direction(match direction {
                "left" => Direction::LEFT,
                "up" => Direction::UP,
                "right" => Direction::RIGHT,
                &_ => Direction::DOWN,
            })
            .finalize()
            .unwrap(),
    )
}

#[tauri::command]
fn zoom(port_state: tauri::State<PortState>, direction: &str) -> () {
    println!("Zoom: {}", direction);

    let mut builder = MessageBuilder::new(1);

    if direction == "in" {
        builder = *builder.zoom_in();
    } else {
        builder = *builder.zoom_out();
    }

    port_state.send_message(builder.finalize().unwrap())
}

#[tauri::command]
fn stop(port_state: tauri::State<PortState>) -> () {
    println!("Stop");

    port_state.send_message(MessageBuilder::new(1).stop().finalize().unwrap())
}

#[tauri::command]
fn get_ports() -> Vec<String> {
    let ports = serialport::available_ports().unwrap_or_else(|_| Vec::new());
    ports
        .into_iter()
        .map(|port| port.port_name.into())
        .collect()
}

fn main() {
    tauri::Builder::default()
        .manage(PortState {
            port: Default::default(),
        })
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            restore_preset,
            get_ports,
            move_camera,
            zoom,
            stop
        ])
        .setup(|app| {
            let app_handle = app.handle();

            app.listen_global("port-changed", move |event| {
                let port_state = app_handle.state::<PortState>();

                port_state.set_port(event.payload().map_or(None, |payload| {
                    match serde_json::from_str::<&str>(payload) {
                        Ok(value) => Some(value),
                        Err(_) => None,
                    }
                }))
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
