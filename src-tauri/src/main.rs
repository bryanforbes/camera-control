// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate log;
extern crate pretty_env_logger;

mod camera;
mod error;
mod ui_state;

use std::sync::Mutex;

use std::{io, process::Command};

use crate::error::Result;

use log::debug;
use pelcodrs::{AutoCtrl, Direction};
use tauri::{
    Manager, WindowEvent,
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tauri_plugin_window_state::StateFlags;
use ui_state::{UIState, UIStateEvent, with_ui_state, with_ui_state_status};

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
    with_ui_state(&app_handle, |ui| ui.refresh_ports());
    open_settings_window(&app_handle)
}

#[tauri::command]
#[specta::specta]
fn get_state(app_handle: tauri::AppHandle) -> Result<UIStateEvent> {
    UIStateEvent::try_from(&app_handle)
}

#[tauri::command]
#[specta::specta]
fn set_port(app_handle: tauri::AppHandle, port_name: Option<&str>) {
    debug!("Port name: {port_name:?}");

    with_ui_state(&app_handle, |ui| ui.set_camera_port(&app_handle, port_name))
}

#[tauri::command]
#[specta::specta]
fn camera_power(app_handle: tauri::AppHandle, power: bool) {
    debug!("Power: {:?}", power);

    with_ui_state_status(
        &app_handle,
        if power { "Power on" } else { "Power off" },
        |ui| {
            let camera = ui.camera()?;

            if power {
                camera.power_on()
            } else {
                camera.power_off()
            }
        },
    )
}

#[tauri::command]
#[specta::specta]
fn autofocus(app_handle: tauri::AppHandle, autofocus: bool) {
    with_ui_state_status(
        &app_handle,
        if autofocus {
            "Autofocus on"
        } else {
            "Autofocus off"
        },
        |ui| {
            ui.camera()?.autofocus(if autofocus {
                AutoCtrl::Auto
            } else {
                AutoCtrl::Off
            })
        },
    )
}

#[tauri::command]
#[specta::specta]
fn go_to_preset(app_handle: tauri::AppHandle, preset: u8, name: &str) {
    debug!("Go To Preset: {}", preset);

    with_ui_state_status(&app_handle, name, |ui| ui.camera()?.go_to_preset(preset));
}

#[tauri::command]
#[specta::specta]
fn set_preset(app_handle: tauri::AppHandle, preset: u8, name: &str) {
    debug!("Set Preset: {}", preset);

    let status = format!("Set {name}");
    with_ui_state_status(&app_handle, &status, |ui| ui.camera()?.set_preset(preset));
}

#[tauri::command]
#[specta::specta]
fn move_camera(app_handle: tauri::AppHandle, direction: &str) {
    debug!("Direction: {}", direction);

    let status = format!("Moving {direction}");
    with_ui_state_status(&app_handle, &status, |ui| {
        ui.camera()?.r#move(match direction {
            "left" => Direction::LEFT,
            "up" => Direction::UP,
            "right" => Direction::RIGHT,
            &_ => Direction::DOWN,
        })
    })
}

#[tauri::command]
#[specta::specta]
fn stop_move(app_handle: tauri::AppHandle) {
    debug!("Stop Move");

    with_ui_state_status(&app_handle, "Done moving", |ui| ui.camera()?.stop())
}

#[tauri::command]
#[specta::specta]
fn zoom(app_handle: tauri::AppHandle, direction: &str) {
    debug!("Zoom: {}", direction);

    let status = format!("Zooming {direction}");
    with_ui_state_status(&app_handle, &status, |ui| {
        let camera = ui.camera()?;

        if direction == "in" {
            camera.zoom_in()
        } else {
            camera.zoom_out()
        }
    });
}

#[tauri::command]
#[specta::specta]
fn stop_zoom(app_handle: tauri::AppHandle) {
    debug!("Stop Zoom");

    with_ui_state_status(&app_handle, "Done zooming", |ui| ui.camera()?.stop());
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

    let specta_builder = tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
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
        .events(tauri_specta::collect_events![UIStateEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Throw);

    #[cfg(debug_assertions)]
    {
        let ts = specta_typescript::Typescript::default()
            .header("/* eslint-disable */ // @ts-nocheck")
            .formatter(|file| {
                Command::new("../node_modules/.bin/prettier")
                    .arg("--write")
                    .arg(file)
                    .output()
                    .map(|_| ())
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            });

        specta_builder
            .export(ts, "../src/lib/bindings.ts")
            .expect("Failed to export typescript bindings");
    }

    #[allow(unused_mut)]
    let mut updater = tauri_plugin_updater::Builder::new();

    #[cfg(target_os = "macos")]
    {
        updater = updater.target("darwin-universal");
    }

    #[allow(unused_mut)]
    let mut tauri_builder = tauri::Builder::default()
        .plugin(updater.build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_prevent_default::debug());

    #[cfg(desktop)]
    {
        tauri_builder = tauri_builder
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                let _ = app
                    .get_webview_window("main")
                    .expect("no main window")
                    .set_focus();
            }))
            .plugin(
                tauri_plugin_window_state::Builder::default()
                    .with_state_flags(StateFlags::POSITION)
                    .build(),
            );
    }

    tauri_builder
        .manage(Mutex::new(UIState::default()))
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    std::process::exit(0);
                }
            }
        })
        .setup(move |app| {
            specta_builder.mount_events(app);

            with_ui_state(app.app_handle(), |ui| ui.initialize(app.handle()));

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
