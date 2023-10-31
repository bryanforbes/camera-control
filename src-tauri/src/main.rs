// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

mod camera_state;
mod error;
mod visca;

use std::ops::Deref;
use std::sync::Mutex;

use camera_state::{CameraState, MutexCameraState};
use log::debug;
use tauri::{AboutMetadata, CustomMenuItem, Manager, Menu, MenuItem, Submenu, WindowEvent, Wry};
use tauri_plugin_store::{with_store, StoreCollection};
use tauri_plugin_window_state::StateFlags;

use crate::error::Result;
use crate::visca::{Autofocus, Focus, Move, Power, Preset, Zoom};

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
fn get_state(
    window: tauri::Window,
    handle: tauri::AppHandle,
    mutex_state: tauri::State<Mutex<CameraState>>,
    stores: tauri::State<StoreCollection<Wry>>,
) -> Result<serde_json::Value> {
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

    Ok(serde_json::to_value(state.deref()).unwrap())
}

#[tauri::command]
#[specta::specta]
fn set_port(
    port: Option<String>,
    handle: tauri::AppHandle,
    mutex_state: tauri::State<Mutex<CameraState>>,
    stores: tauri::State<StoreCollection<Wry>>,
) -> Result<()> {
    debug!("Port name: {:?}", port);

    with_store(handle.app_handle(), stores, "config.json", |store| {
        store.insert("port".into(), port.as_deref().into())?;
        store.save()
    })?;

    let mut state = mutex_state.lock().unwrap();

    /*if port.is_some() {
        state.set_status("Connecting");
        state.send(&handle);
    }*/

    if let Err(error) = state.set_port(port.as_deref()) {
        debug!("Error: {}", error);
        state.set_status(error.to_string());
    }
    state.send(&handle);

    Ok(())
}

#[tauri::command]
fn camera_power(
    mutex_state: tauri::State<Mutex<CameraState>>,
    handle: tauri::AppHandle,
    power: bool,
) {
    let power: Power = power.into();

    debug!("Power: {:?}", power);

    mutex_state.with_state_and_status(&handle, |state| {
        state.set_power(1, power)?;
        Ok(format!("Power {:?}", power))
    });
}

#[tauri::command]
fn autofocus(
    mutex_state: tauri::State<Mutex<CameraState>>,
    handle: tauri::AppHandle,
    autofocus: bool,
) {
    let autofocus: Autofocus = autofocus.into();

    debug!("Autofocus: {:?}", autofocus);

    mutex_state.with_state_and_status(&handle, |state| {
        state.set_autofocus(1, autofocus)?;
        Ok(format!("Autofocus {:?}", autofocus))
    });
}

#[tauri::command]
#[specta::specta]
fn go_to_preset(
    mutex_state: tauri::State<Mutex<CameraState>>,
    handle: tauri::AppHandle,
    preset: u8,
    preset_name: String,
) {
    debug!("Go To Preset: {}", preset);

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(1, Preset::Recall(preset))?;
        Ok(preset_name)
    });
}

#[tauri::command]
#[specta::specta]
fn set_preset(
    mutex_state: tauri::State<Mutex<CameraState>>,
    handle: tauri::AppHandle,
    preset: u8,
    preset_name: String,
) {
    debug!("Set Preset: {}", preset);

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(1, Preset::Set(preset))?;
        Ok(format!("Set {}", preset_name))
    });
}

#[tauri::command]
#[specta::specta]
fn move_camera(
    mutex_state: tauri::State<Mutex<CameraState>>,
    handle: tauri::AppHandle,
    direction: &str,
) {
    debug!("Direction: {}", direction);

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(
            1,
            match direction {
                "left" => Move::Left(1),
                "right" => Move::Right(1),
                "up" => Move::Up(1),
                "down" => Move::Down(1),
                _ => Move::Stop,
            },
        )?;
        Ok(format!("Moving {}", direction))
    });
}

#[tauri::command]
#[specta::specta]
fn stop_move(mutex_state: tauri::State<Mutex<CameraState>>, handle: tauri::AppHandle) {
    debug!("Stop Move");

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(1, Move::Stop)?;
        Ok("Done stopping".into())
    });
}

#[tauri::command]
#[specta::specta]
fn zoom(mutex_state: tauri::State<Mutex<CameraState>>, handle: tauri::AppHandle, direction: &str) {
    debug!("Zoom: {}", direction);

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(1, Zoom::try_from(direction)?)?;
        Ok(format!("Zooming {}", direction))
    });
}

#[tauri::command]
#[specta::specta]
fn stop_zoom(mutex_state: tauri::State<Mutex<CameraState>>, handle: tauri::AppHandle) {
    debug!("Stop Zoom");

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(1, Zoom::Stop)?;
        Ok("Done zooming".into())
    });
}

#[tauri::command]
fn focus(mutex_state: tauri::State<Mutex<CameraState>>, handle: tauri::AppHandle, direction: &str) {
    debug!("Focus: {}", direction);

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(1, Focus::try_from(direction)?)?;
        Ok(format!("Focusing {}", direction))
    });
}

#[tauri::command]
fn stop_focus(mutex_state: tauri::State<Mutex<CameraState>>, handle: tauri::AppHandle) {
    debug!("Stop Focus");

    mutex_state.with_state_and_status(&handle, |state| {
        state.execute(1, Focus::Stop)?;
        Ok("Done focusing".into())
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
    let specta_builder = {
        let builder = tauri_specta::ts::builder()
            .commands(tauri_specta::collect_commands![
                open_settings,
                set_port,
                go_to_preset,
                set_preset,
                move_camera,
                stop_move,
                zoom,
                stop_zoom,
                get_ports
            ])
            .header("// @ts-nocheck\n");

        #[cfg(debug_assertions)]
        let builder = builder.path("../src/commands.ts");

        builder.into_plugin()
    };

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
        .manage(Mutex::new(CameraState::default()))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(specta_builder)
        .invoke_handler(tauri::generate_handler![
            get_state,
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
            focus,
            stop_focus,
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
