// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate log;
extern crate pretty_env_logger;

mod camera;
mod error;
mod pelco_camera;
mod ui_state;
#[cfg(feature = "visca")]
mod visca;

use std::sync::Mutex;

use crate::error::Result;

use camera::{Direction, Zoom};
use log::debug;
#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Manager, WindowEvent};
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
        |ui| ui.camera()?.autofocus(autofocus),
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
fn move_camera(app_handle: tauri::AppHandle, direction: Direction) {
    debug!("Direction: {}", direction);

    let status = format!("Moving {direction}");
    with_ui_state_status(&app_handle, &status, |ui| ui.camera()?.pan_tilt(direction))
}

#[tauri::command]
#[specta::specta]
fn stop_move(app_handle: tauri::AppHandle) {
    debug!("Stop Move");

    with_ui_state_status(&app_handle, "Done moving", |ui| ui.camera()?.stop())
}

#[tauri::command]
#[specta::specta]
fn zoom(app_handle: tauri::AppHandle, direction: Zoom) {
    debug!("Zoom: {}", direction);

    let status = format!("Zooming {direction}");
    with_ui_state_status(&app_handle, &status, |ui| {
        let camera = ui.camera()?;

        camera.zoom(direction)
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
fn refresh_ports(app_handle: tauri::AppHandle) {
    debug!("Refreshing ports");

    with_ui_state_status(&app_handle, "Ports refreshed", |ui| ui.refresh_ports());
}

fn stop_camera_before_exit(app_handle: &tauri::AppHandle) {
    with_ui_state(app_handle, |ui| {
        if let Ok(camera) = ui.camera() {
            camera.stop()?;
        }
        Ok(())
    });
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

    let specta_builder = tauri_specta::Builder::new()
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
            refresh_ports,
        ])
        .events(tauri_specta::collect_events![UIStateEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Throw);

    #[cfg(debug_assertions)]
    {
        let ts =
            specta_typescript::Typescript::default().header("/* eslint-disable */ // @ts-nocheck");

        specta_builder
            .export(ts, "../src/lib/bindings.ts")
            .expect("Failed to export typescript bindings");

        std::process::Command::new("pnpm")
            .current_dir("../")
            .args(["exec", "prettier", "--write", "./src/lib/bindings.ts"])
            .status()
            .expect("Failed to run prettier on typescript bindings");
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

    let app = tauri_builder
        .manage(Mutex::new(UIState::default()))
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } => {
                stop_camera_before_exit(window.app_handle());
            }
            WindowEvent::Destroyed if window.label() == "main" => {
                window.app_handle().exit(0);
            }
            _ => {}
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
                            check_for_updates(&handle, true).await;
                        });
                    }
                });
            }

            let handle = app.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                check_for_updates(&handle, false).await;
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            stop_camera_before_exit(app_handle);
        }
    });
}

fn report_update_error(
    app: &tauri::AppHandle,
    title: &str,
    error: &tauri_plugin_updater::Error,
    notify: bool,
) {
    log::error!("{title}: {error}");

    if notify {
        app.dialog()
            .message(format!(
                "{title}.\n\n\
                 Check your internet connection and try again. \
                 If the problem persists, contact the application maintainer.\n\n\
                 Details: {error}"
            ))
            .title(title)
            .kind(MessageDialogKind::Error)
            .buttons(MessageDialogButtons::Ok)
            .show(|_| {});
    }
}

async fn check_for_updates(app: &tauri::AppHandle, notify_up_to_date: bool) {
    let app_name = &app.package_info().name;

    let result = async { app.updater()?.check().await }.await;

    let update = match result {
        Ok(update) => update,
        Err(error) => {
            report_update_error(
                app,
                "Failed to check for updates",
                &error,
                notify_up_to_date,
            );
            return;
        }
    };

    if let Some(update) = update {
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
            if let Err(error) = update
                .download_and_install(
                    |chunk_length, content_length| {
                        downloaded += chunk_length;
                        debug!("downloaded {downloaded} from {content_length:?}");
                    },
                    || {
                        debug!("download finished");
                    },
                )
                .await
            {
                report_update_error(app, "Failed to install update", &error, true);
                return;
            }

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
}
