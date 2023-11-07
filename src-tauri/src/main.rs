// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate log;
extern crate pretty_env_logger;

mod error;
mod port_state;

use crate::error::Result;

use log::debug;
use pelcodrs::{AutoCtrl, Direction, Message, MessageBuilder, Speed};
use port_state::{with_port, PortState};
use tauri::{AboutMetadata, CustomMenuItem, Manager, Menu, MenuItem, Submenu, WindowEvent, Wry};
use tauri_plugin_store::StoreCollection;
use tauri_plugin_window_state::StateFlags;

fn open_settings_window(app_handle: tauri::AppHandle) -> Result<()> {
    if let Some(window) = app_handle.get_window("settings") {
        window.set_focus()?;
    } else {
        tauri::WindowBuilder::new(
            &app_handle,
            "settings",
            tauri::WindowUrl::App("settings.html".into()),
        )
        .title("Camera Control Settings")
        .resizable(false)
        .accept_first_mouse(true)
        .inner_size(600.0, 480.0)
        .build()?;
    }
    Ok(())
}

#[tauri::command]
async fn open_settings(app_handle: tauri::AppHandle) -> Result<()> {
    open_settings_window(app_handle)
}

#[tauri::command]
fn ready(window: tauri::Window, port_state: tauri::State<PortState>) -> Result<()> {
    with_port(port_state, |port| port.emit(&window))
}

#[tauri::command]
fn set_port(
    port_name: Option<String>,
    handle: tauri::AppHandle,
    port_state: tauri::State<PortState>,
    stores: tauri::State<StoreCollection<Wry>>,
) -> Result<()> {
    debug!("Port name: {:?}", port_name);

    with_port(port_state, |port| {
        port.set(handle.app_handle(), stores, port_name.as_deref())
    })
}

#[tauri::command]
fn camera_power(port_state: tauri::State<PortState>, power: bool) -> Result<()> {
    debug!("Power: {:?}", power);

    let mut builder = MessageBuilder::new(1);

    if power {
        builder = *builder.camera_on();
    } else {
        builder = *builder.camera_off();
    }

    with_port(port_state, |port| port.send_message(builder.finalize()?))
}

#[tauri::command]
fn autofocus(port_state: tauri::State<PortState>, autofocus: bool) -> Result<()> {
    with_port(port_state, |port| {
        port.send_message(Message::auto_focus(
            1,
            if autofocus {
                AutoCtrl::Auto
            } else {
                AutoCtrl::Off
            },
        )?)
    })
}

#[tauri::command]
fn go_to_preset(port_state: tauri::State<PortState>, preset: u8) -> Result<()> {
    debug!("Go To Preset: {}", preset);

    with_port(port_state, |port| {
        port.send_message(Message::go_to_preset(1, preset)?)
    })
}

#[tauri::command]
fn set_preset(port_state: tauri::State<PortState>, preset: u8) -> Result<()> {
    debug!("Set Preset: {}", preset);

    with_port(port_state, |port| {
        port.send_message(Message::set_preset(1, preset)?)
    })
}

#[tauri::command]
fn move_camera(port_state: tauri::State<PortState>, direction: &str) -> Result<()> {
    debug!("Direction: {}", direction);

    with_port(port_state, |port| {
        port.send_message(
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
    })
}

#[tauri::command]
fn stop_move(port_state: tauri::State<PortState>) -> Result<()> {
    debug!("Stop Move");

    with_port(port_state, |port| {
        port.send_message(MessageBuilder::new(1).stop().finalize()?)
    })
}

#[tauri::command]
fn zoom(port_state: tauri::State<PortState>, direction: &str) -> Result<()> {
    debug!("Zoom: {}", direction);

    let mut builder = MessageBuilder::new(1);

    if direction == "in" {
        builder = *builder.zoom_in();
    } else {
        builder = *builder.zoom_out();
    }

    with_port(port_state, |port| port.send_message(builder.finalize()?))
}

#[tauri::command]
fn stop_zoom(port_state: tauri::State<PortState>) -> Result<()> {
    debug!("Stop Zoom");

    with_port(port_state, |port| {
        port.send_message(MessageBuilder::new(1).stop().finalize()?)
    })
}

#[tauri::command]
fn get_ports() -> Result<Vec<String>> {
    Ok(serialport::available_ports()?
        .into_iter()
        .map(|port| port.port_name)
        .collect())
}

fn main() {
    pretty_env_logger::formatted_builder()
        .filter(
            Some("camera_control"),
            if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Error
            },
        )
        .init();

    let context = tauri::generate_context!();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    {
        let app_name: &str = &context.package_info().name;

        builder = builder
            .updater_target("darwin-universal")
            .menu(
                Menu::new()
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
                    )),
            )
            .on_menu_event(|event| match event.menu_item_id() {
                "settings" => {
                    open_settings_window(event.window().app_handle()).unwrap_or_default();
                }
                "check-for-updates" => {
                    event.window().trigger("tauri://update", None);
                }
                _ => {}
            });
    }

    builder
        .manage(PortState::default())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            ready,
            set_port,
            open_settings,
            camera_power,
            autofocus,
            go_to_preset,
            set_preset,
            get_ports,
            move_camera,
            stop_move,
            zoom,
            stop_zoom,
        ])
        .on_window_event(|event| {
            if let WindowEvent::Destroyed = event.event() {
                if event.window().label() == "main" {
                    std::process::exit(0);
                }
            }
        })
        .setup(|app| {
            let port_state: tauri::State<PortState> = app.state();
            let stores: tauri::State<StoreCollection<Wry>> = app.state();

            with_port(port_state, |port| port.initialize(app.app_handle(), stores))?;

            Ok(())
        })
        .run(context)
        .expect("error while running tauri application")
}
