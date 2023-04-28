// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod port_state;

use pelcodrs::{AutoCtrl, Direction, Message, MessageBuilder, Speed};
use tauri::utils::assets::EmbeddedAssets;
use tauri::{
    AboutMetadata, Context, CustomMenuItem, Manager, Menu, MenuItem, Submenu, WindowEvent, Wry,
};
use tauri_plugin_window_state::StateFlags;

use crate::error::Result;
use crate::port_state::PortState;

fn send_staus(app_handle: &tauri::AppHandle, status: &str) -> () {
    app_handle.emit_to("main", "status", status).ok();
}

#[tauri::command]
fn camera_power(port_state: tauri::State<PortState>, state: bool) -> Result<()> {
    println!("Power: {}", state);

    let mut builder = MessageBuilder::new(1);

    if state {
        builder = *builder.camera_on();
    } else {
        builder = *builder.camera_off();
    }

    port_state.send_message(builder.finalize()?)
}

#[tauri::command]
fn autofocus(port_state: tauri::State<PortState>, state: bool) -> Result<()> {
    println!("Autofocus: {}", state);

    port_state.send_message(Message::auto_focus(
        1,
        if state { AutoCtrl::Auto } else { AutoCtrl::Off },
    )?)
}

#[tauri::command]
fn go_to_preset(port_state: tauri::State<PortState>, preset: u8) -> Result<()> {
    println!("Go To Preset: {}", preset);

    port_state.send_message(Message::go_to_preset(1, preset)?)
}

#[tauri::command]
fn set_preset(port_state: tauri::State<PortState>, preset: u8) -> Result<()> {
    println!("Set Preset: {}", preset);

    port_state.send_message(Message::set_preset(1, preset)?)
}

#[tauri::command]
fn move_camera(port_state: tauri::State<PortState>, direction: &str) -> Result<()> {
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
            .finalize()?,
    )
}

#[tauri::command]
fn zoom(port_state: tauri::State<PortState>, direction: &str) -> Result<()> {
    println!("Zoom: {}", direction);

    let mut builder = MessageBuilder::new(1);

    if direction == "in" {
        builder = *builder.zoom_in();
    } else {
        builder = *builder.zoom_out();
    }

    port_state.send_message(builder.finalize()?)
}

#[tauri::command]
fn stop(port_state: tauri::State<PortState>) -> Result<()> {
    println!("Stop");

    port_state.send_message(MessageBuilder::new(1).stop().finalize()?)
}

#[tauri::command]
fn get_ports() -> Result<Vec<String>> {
    Ok(serialport::available_ports()?
        .into_iter()
        .map(|port| port.port_name.to_string())
        .collect())
}

fn create_builder(context: &Context<EmbeddedAssets>) -> tauri::Builder<Wry> {
    let builder = tauri::Builder::default();

    if cfg!(target_os = "macos") {
        let app_name: &str = &context.package_info().name;
        let menu = Menu::new()
            .add_submenu(Submenu::new(
                app_name,
                Menu::new()
                    .add_native_item(MenuItem::About(
                        app_name.to_string(),
                        AboutMetadata::default(),
                    ))
                    .add_item(CustomMenuItem::new(
                        "check-for-updates".to_string(),
                        "Check for updates...",
                    ))
                    .add_native_item(MenuItem::Separator)
                    .add_item(
                        CustomMenuItem::new("settings".to_string(), "Settings")
                            .accelerator("cmd+,"),
                    )
                    .add_native_item(MenuItem::Separator)
                    .add_native_item(MenuItem::Services)
                    .add_native_item(MenuItem::Separator)
                    .add_native_item(MenuItem::Hide)
                    .add_native_item(MenuItem::HideOthers)
                    .add_native_item(MenuItem::ShowAll)
                    .add_native_item(MenuItem::Separator)
                    .add_native_item(MenuItem::Quit),
            ))
            .add_submenu(Submenu::new(
                "Window",
                Menu::new()
                    .add_native_item(MenuItem::Minimize)
                    .add_native_item(MenuItem::Zoom)
                    .add_native_item(MenuItem::CloseWindow),
            ));

        builder
            .menu(menu)
            .on_menu_event(|event| match event.menu_item_id() {
                "settings" => {
                    event.window().emit("open-settings", "").unwrap_or(());
                }
                "check-for-updates" => {
                    event.window().trigger("tauri://update", None);
                }
                _ => {}
            })
    } else {
        builder
    }
}

fn main() {
    let context = tauri::generate_context!();

    create_builder(&context)
        .manage(PortState::new())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            camera_power,
            autofocus,
            go_to_preset,
            set_preset,
            get_ports,
            move_camera,
            zoom,
            stop
        ])
        .on_window_event(|event| match event.event() {
            WindowEvent::Destroyed => {
                if event.window().label() == "main" {
                    std::process::exit(0);
                }
            }
            _ => {}
        })
        .setup(|app| {
            let app_handle = app.handle();

            app.listen_global("port-changed", move |event| {
                let port = event
                    .payload()
                    .map(|payload| serde_json::from_str::<&str>(payload).ok())
                    .flatten();

                send_staus(
                    &app_handle,
                    match port {
                        Some(_) => "Connecting",
                        None => "Disconnecting",
                    },
                );

                let port_state = app_handle.state::<PortState>();

                if let Err(error) = port_state.set_port(port) {
                    app_handle
                        .emit_all("port-change-error", error.to_string())
                        .unwrap_or(())
                } else {
                    send_staus(
                        &app_handle,
                        match port {
                            Some(_) => "Connected",
                            None => "Disconnected",
                        },
                    )
                }
            });

            Ok(())
        })
        .run(context)
        .expect("error while running tauri application")
}
