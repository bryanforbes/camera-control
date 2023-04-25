// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use pelcodrs::{Direction, Message, MessageBuilder, PelcoDPort, Speed};
use serde_json::Value as JsonValue;
use serialport::{DataBits, SerialPort, StopBits};
use tauri::{Manager, Wry};
use tauri_plugin_store::{with_store, StoreCollection};

struct PortState {
    port: Mutex<Option<PelcoDPort<Box<dyn SerialPort>>>>,
}

fn set_port<'a>(port_state: tauri::State<PortState>, path: Option<JsonValue>) -> () {
    let path = path.and_then(|port| {
        if port.is_null() {
            None
        } else {
            Some(port.as_str().unwrap().to_owned())
        }
    });
    *port_state.port.lock().unwrap() = match path {
        Some(path) => Some(PelcoDPort::new(
            serialport::new(path, 9000)
                .stop_bits(StopBits::One)
                .data_bits(DataBits::Eight)
                .open()
                .expect("Poop"),
        )),
        None => None,
    };
}

fn send_message(port_state: tauri::State<PortState>, message: Message) -> () {
    port_state
        .port
        .lock()
        .unwrap()
        .as_mut()
        .expect("The port is not set")
        .send_message(message)
        .expect("Something went wrong sending message")
}

#[tauri::command]
fn restore_preset(port_state: tauri::State<PortState>, preset: u8) -> () {
    send_message(port_state, Message::go_to_preset(1, preset).unwrap())
}

#[tauri::command]
fn move_camera(port_state: tauri::State<PortState>, direction: &str) -> () {
    println!("Direction: {}", direction);
    send_message(
        port_state,
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

    send_message(port_state, builder.finalize().unwrap())
}

#[tauri::command]
fn stop(port_state: tauri::State<PortState>) -> () {
    println!("Stop");
    send_message(
        port_state,
        MessageBuilder::new(1).stop().finalize().unwrap(),
    )
}

#[tauri::command]
fn get_ports() -> Vec<String> {
    let ports = serialport::available_ports().unwrap();
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
            app.listen_global("port-changed", |event| {
                // TODO: how to set the port in the managed state?
                let payload = match event.payload() {
                    Some(payload) => match serde_json::from_str::<JsonValue>(payload) {
                        Ok(p) => Some(p),
                        Err(_) => None,
                    },
                    None => None,
                };
                println!("Event: {:?}", payload);
            });

            let port_state = app.state::<PortState>();

            let stores = app.state::<StoreCollection<Wry>>();
            let path = PathBuf::from("config.json");

            let config_port = with_store(app.handle(), stores, path, |store| {
                Ok(store.get("port").cloned())
            })
            .unwrap();

            set_port(port_state, config_port);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
