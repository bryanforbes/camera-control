// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate log;
extern crate pretty_env_logger;

mod error;
mod ui_state;

use std::sync::Mutex;

use std::{io, process::Command};

use crate::error::Result;

use log::debug;
use pelcodrs::{AutoCtrl, Direction, Message, MessageBuilder, Speed};
use specta_typescript::Typescript;
use tauri::{
    Manager, WindowEvent,
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tauri_plugin_window_state::StateFlags;
use tauri_specta::{Builder, collect_commands, collect_events};
use ui_state::{UIState, UIStateEvent, with_ui_state};

fn open_settings_window(app_handle: &tauri::AppHandle) -> Result<()> {
    if let Some(window) = app_handle.get_webview_window("settings") {
        window.set_focus()?;
    } else if let Some(config) = app_handle
        .config()
        .app
        .windows
        .iter()
        .find(|item| item.label == "settings")
    {
        tauri::WebviewWindowBuilder::from_config(app_handle, config)?.build()?;
    }
    Ok(())
}

// This command MUST be async as per Tauri's documentation at
// https://tauri.app/v1/guides/features/multiwindow#create-a-window-using-an-apphandle-instance
#[tauri::command]
#[specta::specta]
async fn open_settings(app_handle: tauri::AppHandle) -> Result<()> {
    with_ui_state(&app_handle, |ui| ui.populate_ports())?;
    open_settings_window(&app_handle)
}

#[tauri::command]
#[specta::specta]
fn get_state(app_handle: tauri::AppHandle) -> Result<UIStateEvent> {
    UIStateEvent::try_from(&app_handle)
}

#[tauri::command]
#[specta::specta]
fn set_port(app_handle: tauri::AppHandle, port_name: Option<&str>) -> Result<()> {
    debug!("Port name: {port_name:?}");

    with_ui_state(&app_handle, |ui| ui.set_port(&app_handle, port_name))
}

#[tauri::command]
#[specta::specta]
fn camera_power(app_handle: tauri::AppHandle, power: bool) -> Result<()> {
    debug!("Power: {:?}", power);

    let mut builder = MessageBuilder::new(1);

    if power {
        builder = *builder.camera_on();
    } else {
        builder = *builder.camera_off();
    }

    with_ui_state(&app_handle, |ui| ui.port.send_message(builder.finalize()?))
}

#[tauri::command]
#[specta::specta]
fn autofocus(app_handle: tauri::AppHandle, autofocus: bool) -> Result<()> {
    with_ui_state(&app_handle, |ui| {
        ui.port.send_message(Message::auto_focus(
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
#[specta::specta]
fn go_to_preset(app_handle: tauri::AppHandle, preset: u8, name: &str) -> Result<()> {
    debug!("Go To Preset: {}", preset);

    with_ui_state(&app_handle, |ui| {
        ui.port.send_message(Message::go_to_preset(1, preset)?)?;
        ui.set_status(name)
    })
}

#[tauri::command]
#[specta::specta]
fn set_preset(app_handle: tauri::AppHandle, preset: u8, name: &str) -> Result<()> {
    debug!("Set Preset: {}", preset);

    with_ui_state(&app_handle, |ui| {
        ui.port.send_message(Message::go_to_preset(1, preset)?)?;
        let status = format!("Set {name}");
        ui.set_status(&status)
    })
}

#[tauri::command]
#[specta::specta]
fn move_camera(app_handle: tauri::AppHandle, direction: &str) -> Result<()> {
    debug!("Direction: {}", direction);

    with_ui_state(&app_handle, |ui| {
        ui.port.send_message(
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
        let status = format!("Moving {direction}");
        ui.set_status(&status)
    })
}

#[tauri::command]
#[specta::specta]
fn stop_move(app_handle: tauri::AppHandle) -> Result<()> {
    debug!("Stop Move");

    with_ui_state(&app_handle, |ui| {
        ui.port
            .send_message(MessageBuilder::new(1).stop().finalize()?)?;
        ui.set_status("Done moving")
    })
}

#[tauri::command]
#[specta::specta]
fn zoom(app_handle: tauri::AppHandle, direction: &str) -> Result<()> {
    debug!("Zoom: {}", direction);

    let mut builder = MessageBuilder::new(1);

    if direction == "in" {
        builder = *builder.zoom_in();
    } else {
        builder = *builder.zoom_out();
    }

    with_ui_state(&app_handle, |ui| {
        ui.port.send_message(builder.finalize()?)?;
        let status = format!("Zooming {direction}");
        ui.set_status(&status)
    })
}

#[tauri::command]
#[specta::specta]
fn stop_zoom(app_handle: tauri::AppHandle) -> Result<()> {
    debug!("Stop Zoom");

    with_ui_state(&app_handle, |ui| {
        ui.port
            .send_message(MessageBuilder::new(1).stop().finalize()?)?;
        ui.set_status("Done zooming")
    })
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

    #[allow(unused_mut)]
    let mut updater = tauri_plugin_updater::Builder::new();

    #[cfg(target_os = "macos")]
    {
        updater = updater.target("darwin-universal");
    }

    #[allow(unused_mut)]
    let mut builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            open_settings,
            get_state,
            set_port,
            camera_power,
            autofocus,
            go_to_preset,
            set_preset,
            move_camera,
            stop_move,
            zoom,
            stop_zoom,
            get_ports,
        ])
        .events(collect_events![UIStateEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Throw);

    #[cfg(debug_assertions)]
    {
        let ts = Typescript::default()
            .header("/* eslint-disable */ // @ts-nocheck")
            .formatter(|file| {
                Command::new("../node_modules/.bin/prettier")
                    .arg("--write")
                    .arg(file)
                    .output()
                    .map(|_| ())
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            });

        builder
            .export(ts, "../src/lib/bindings.ts")
            .expect("Failed to export typescript bindings");
    }

    tauri::Builder::default()
        .plugin(updater.build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(UIState::default()))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .build(),
        )
        .invoke_handler(builder.invoke_handler())
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    std::process::exit(0);
                }
            }
        })
        .setup(move |app| {
            builder.mount_events(app);

            with_ui_state(app.app_handle(), |ui| ui.initialize(app.handle()))?;

            #[cfg(target_os = "macos")]
            {
                let app_name = &app.package_info().name;

                let app_menu = SubmenuBuilder::new(app, app_name)
                    .about_with_text(format!("About {}", app_name), None)
                    .item(
                        &MenuItemBuilder::with_id("check-for-updates", "Check for updates...")
                            .build(app)?,
                    )
                    .separator()
                    .item(
                        &MenuItemBuilder::with_id("settings", "Settings")
                            .accelerator("cmd+,")
                            .build(app)?,
                    )
                    .separator()
                    .services()
                    .separator()
                    .hide_with_text(format!("Hide {}", app_name))
                    .hide_others()
                    .show_all()
                    .quit_with_text(format!("Quit {}", app_name))
                    .build()?;

                let window_menu = SubmenuBuilder::new(app, "Window")
                    .minimize()
                    .maximize()
                    .close_window()
                    .build()?;

                let menu = MenuBuilder::new(app)
                    .item(&app_menu)
                    .item(&window_menu)
                    .build()?;

                app.set_menu(menu)?;

                app.on_menu_event(|app, event| {
                    if event.id() == "settings" {
                        open_settings_window(app).unwrap_or_default();
                    } else if event.id() == "check-for-updates" {
                        let handle = app.app_handle().clone();
                        tauri::async_runtime::spawn(async move {
                            check_for_updates(&handle, true).await.unwrap();
                        });
                    }
                });
            }

            let handle = app.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                check_for_updates(&handle, false).await.unwrap();
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn check_for_updates(
    app: &tauri::AppHandle,
    notify_up_to_date: bool,
) -> tauri_plugin_updater::Result<()> {
    let app_name = &app.package_info().name;

    if let Some(update) = app.updater()?.check().await? {
        let body = update.body.clone().unwrap_or_else(|| String::from(""));

        let should_install = app
            .dialog()
            .message(format!(
                r#"{app_name} {} is now available -- you have {}.

Would you like to install it now?

Release Notes:
{body}"#,
                update.version, update.current_version
            ))
            .title(format!(r#"A new version of {app_name} is available!"#))
            .kind(MessageDialogKind::Info)
            .buttons(MessageDialogButtons::YesNo)
            .blocking_show();

        if should_install {
            let mut downloaded = 0;

            // alternatively we could also call update.download() and update.install() separately
            update
                .download_and_install(
                    |chunk_length, content_length| {
                        downloaded += chunk_length;
                        debug!("downloaded {downloaded} from {content_length:?}");
                    },
                    || {
                        debug!("download finished");
                    },
                )
                .await?;

            let should_exit = app
                .dialog()
                .message(
                    "The installation was successful. Do you want to restart the application now?",
                )
                .title("Ready to Restart")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::YesNo)
                .blocking_show();

            if should_exit {
                app.restart();
            }
        }
    } else if notify_up_to_date {
        app.dialog()
            .message(format!("{app_name} is already up to date!"))
            .kind(MessageDialogKind::Info)
            .title("Up to Date")
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
    }

    Ok(())
}
