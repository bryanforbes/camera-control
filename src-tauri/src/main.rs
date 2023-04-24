// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[tauri::command]
fn restore_preset(preset: &str) -> () {
    println!("Preset: {}", preset)
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
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![greet, restore_preset, get_ports])
        .setup(|app| {
            app.listen_global("store://change", move |event| {
                println!("got store://change with payload {:?}", event.payload());
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}
