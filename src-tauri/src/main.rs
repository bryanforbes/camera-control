// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate log;
extern crate pretty_env_logger;

mod error;
mod port_state;

use std::ops::Deref;
use std::sync::Mutex;

use log::debug;
use pelcodrs::{AutoCtrl, Direction, Message, MessageBuilder, Speed};
use port_state::{MutexPortState, PortState, PortStateEvent};
use tauri::{AboutMetadata, CustomMenuItem, Manager, Menu, MenuItem, Submenu, WindowEvent, Wry};
use tauri_plugin_store::{with_store, StoreCollection};
use tauri_plugin_window_state::StateFlags;
use tauri_specta::Event;

use crate::error::Result;

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
#[specta::specta]
async fn open_settings(app_handle: tauri::AppHandle) -> Result<()> {
    open_settings_window(app_handle)
}

#[tauri::command]
#[specta::specta]
fn ready(
    window: tauri::Window,
    handle: tauri::AppHandle,
    mutex_state: tauri::State<Mutex<PortState>>,
    stores: tauri::State<StoreCollection<Wry>>,
) -> Result<()> {
    let mut state = mutex_state.lock().unwrap();

    if window.label() == "main" {
        let port_name = with_store(handle, stores, "config.json", |store| {
            Ok(store.get("port").cloned())
        })?;

        if let Some(port_name) = port_name {
            if let Err(error) = state.set_port(port_name.as_str()) {
                debug!("Error: {}", error);
                state.set_status(error.to_string());
            }
        }
    }

    PortStateEvent::from(state.deref()).emit(&window).ok();

    Ok(())
}

#[tauri::command]
#[specta::specta]
fn set_port(
    port: Option<String>,
    handle: tauri::AppHandle,
    mutex_state: tauri::State<Mutex<PortState>>,
    stores: tauri::State<StoreCollection<Wry>>,
) -> Result<()> {
    debug!("Port name: {:?}", port);

    with_store(handle.app_handle(), stores, "config.json", |store| {
        store.insert("port".into(), port.as_deref().into())?;
        store.save()
    })?;

    let mut state = mutex_state.lock().unwrap();

    if let Err(error) = state.set_port(port.as_deref()) {
        debug!("Error: {}", error);
        state.set_status(error.to_string());
    }

    PortStateEvent::from(state.deref()).emit_all(&handle).ok();

    Ok(())
}

#[tauri::command]
#[specta::specta]
fn camera_power(
    mutex_state: tauri::State<Mutex<PortState>>,
    handle: tauri::AppHandle,
    power: bool,
) {
    debug!("Power: {:?}", power);

    mutex_state.with_state_and_status(&handle, |state| {
        let mut builder = MessageBuilder::new(1);

        if power {
            builder = *builder.camera_on();
        } else {
            builder = *builder.camera_off();
        }

        state.send_message(builder.finalize()?)?;

        Ok(format!("Power {:?}", if power { "on" } else { "off" }))
    });
}

#[tauri::command]
#[specta::specta]
fn autofocus(
    mutex_state: tauri::State<Mutex<PortState>>,
    handle: tauri::AppHandle,
    autofocus: bool,
) {
    debug!("Autofocus: {:?}", autofocus);

    mutex_state.with_state_and_status(&handle, |state| {
        state.send_message(Message::auto_focus(
            1,
            if autofocus {
                AutoCtrl::Auto
            } else {
                AutoCtrl::Off
            },
        )?)?;
        Ok(format!(
            "Autofocus {:?}",
            if autofocus { "on" } else { "off" }
        ))
    });
}

#[tauri::command]
#[specta::specta]
fn go_to_preset(
    mutex_state: tauri::State<Mutex<PortState>>,
    handle: tauri::AppHandle,
    preset: u8,
    preset_name: String,
) {
    debug!("Go To Preset: {}", preset);

    mutex_state.with_state_and_status(&handle, |state| {
        state.send_message(Message::go_to_preset(1, preset)?)?;
        Ok(preset_name)
    });
}

#[tauri::command]
#[specta::specta]
fn set_preset(
    mutex_state: tauri::State<Mutex<PortState>>,
    handle: tauri::AppHandle,
    preset: u8,
    preset_name: String,
) {
    debug!("Set Preset: {}", preset);

    mutex_state.with_state_and_status(&handle, |state| {
        state.send_message(Message::set_preset(1, preset)?)?;
        Ok(format!("Set {}", preset_name))
    });
}

#[tauri::command]
#[specta::specta]
fn move_camera(
    mutex_state: tauri::State<Mutex<PortState>>,
    handle: tauri::AppHandle,
    direction: &str,
) {
    debug!("Direction: {}", direction);

    mutex_state.with_state_and_status(&handle, |state| {
        state.send_message(
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
        )?;
        Ok(format!("Moving {}", direction))
    });
}

#[tauri::command]
#[specta::specta]
fn stop_move(mutex_state: tauri::State<Mutex<PortState>>, handle: tauri::AppHandle) {
    debug!("Stop Move");

    mutex_state.with_state_and_status(&handle, |state| {
        state.send_message(MessageBuilder::new(1).stop().finalize()?)?;
        Ok("Done moving".into())
    });
}

#[tauri::command]
#[specta::specta]
fn zoom(mutex_state: tauri::State<Mutex<PortState>>, handle: tauri::AppHandle, direction: &str) {
    debug!("Zoom: {}", direction);

    mutex_state.with_state_and_status(&handle, |state| {
        let mut builder = MessageBuilder::new(1);

        if direction == "in" {
            builder = *builder.zoom_in();
        } else {
            builder = *builder.zoom_out();
        }

        state.send_message(builder.finalize()?)?;
        Ok(format!("Zooming {}", direction))
    });
}

#[tauri::command]
#[specta::specta]
fn stop_zoom(mutex_state: tauri::State<Mutex<PortState>>, handle: tauri::AppHandle) {
    debug!("Stop Zoom");

    mutex_state.with_state_and_status(&handle, |state| {
        state.send_message(MessageBuilder::new(1).stop().finalize()?)?;
        Ok("Done zooming".into())
    });
}

#[tauri::command]
#[specta::specta]
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
                    open_settings_window(event.window().app_handle()).unwrap_or(());
                }
                "check-for-updates" => {
                    event.window().trigger("tauri://update", None);
                }
                _ => {}
            });
    }

    builder
        .manage(Mutex::new(PortState::default()))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin({
            let builder = tauri_specta::ts::builder()
                .commands(tauri_specta::collect_commands![
                    open_settings,
                    ready,
                    set_port,
                    camera_power,
                    autofocus,
                    go_to_preset,
                    set_preset,
                    move_camera,
                    stop_move,
                    zoom,
                    stop_zoom,
                    get_ports
                ])
                .events(tauri_specta::collect_events![PortStateEvent])
                .header("// @ts-nocheck\n");

            #[cfg(debug_assertions)]
            let builder = builder.path("../src/commands.ts");

            builder.into_plugin()
        })
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
        .run(context)
        .expect("error while running tauri application")
}
