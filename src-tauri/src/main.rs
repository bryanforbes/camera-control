// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate log;
extern crate pretty_env_logger;

mod camera;
mod error;
#[cfg(feature = "pelco")]
mod pelco_camera;
mod ui_state;
#[cfg(feature = "visca")]
mod visca;

use crate::{
    camera::{CameraEvent, Direction, Zoom},
    error::Result,
    ui_state::{
        CameraEventSenderState, PortChangeLock, UIState, UIStateEvent, initialize_ui_state,
        publish_camera_event, set_ui_state_session, shutdown_ui_state, stop_ui_state_motion,
        with_ui, with_ui_emit,
    },
};
use log::debug;
#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Manager, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tauri_plugin_window_state::StateFlags;
use tokio::sync::mpsc;

fn open_settings_window(app: &tauri::AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window.set_focus()?;
    } else if let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|item| item.label == "settings")
    {
        tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
    }
    Ok(())
}

// This command MUST be async as per Tauri's documentation at
// https://tauri.app/v1/guides/features/multiwindow#create-a-window-using-an-apphandle-instance
#[tauri::command]
#[specta::specta]
async fn open_settings(app: tauri::AppHandle) -> Result<()> {
    let _ = with_ui_emit(&app, |ui| ui.refresh_ports());
    open_settings_window(&app)
}

#[tauri::command]
#[specta::specta]
fn get_state(app: tauri::AppHandle) -> Result<UIStateEvent> {
    with_ui(&app, |ui| Ok(UIStateEvent::from(&*ui)))
}

#[tauri::command]
#[specta::specta]
async fn set_port(app: tauri::AppHandle, port_name: Option<&str>) -> Result<()> {
    debug!("Port name: {port_name:?}");

    set_ui_state_session(&app, port_name).await?;

    Ok(())
}

/*#[tauri::command]
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
}*/

/*#[tauri::command]
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
}*/

#[tauri::command]
#[specta::specta]
async fn go_to_preset(app: tauri::AppHandle, preset: u8, name: &str) -> Result<()> {
    debug!("Go To Preset: {}", preset);

    let camera = with_ui(&app, |ui| ui.camera())?;
    camera.recall_preset(preset, name).await.map(|_| ())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn set_preset(app: tauri::AppHandle, preset: u8, name: &str) -> Result<()> {
    debug!("Set Preset: {}", preset);

    let camera = with_ui(&app, |ui| ui.camera())?;
    camera.set_preset(preset, name).await.map(|_| ())?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn move_camera(app: tauri::AppHandle, direction: Direction) -> Result<String> {
    debug!("Direction: {}", direction);
    with_ui(&app, |ui| {
        ui.start_motion(|camera| camera.pan_tilt(direction))
    })
}

#[tauri::command]
#[specta::specta]
async fn zoom(app: tauri::AppHandle, direction: Zoom) -> Result<String> {
    debug!("Zoom: {}", direction);
    with_ui(&app, |ui| ui.start_motion(|camera| camera.zoom(direction)))
}

#[tauri::command]
#[specta::specta]
async fn stop_motion(app: tauri::AppHandle, token: &str) -> Result<()> {
    stop_ui_state_motion(&app, token).await
}

#[tauri::command]
#[specta::specta]
fn refresh_ports(app: tauri::AppHandle) {
    debug!("Refreshing ports");

    let _ = with_ui_emit(&app, |ui| {
        ui.refresh_ports()?;
        ui.set_status("Ports refreshed");
        Ok(())
    });
}

fn stop_camera_before_exit(app: &tauri::AppHandle) {
    tauri::async_runtime::block_on(shutdown_ui_state(app)).unwrap_or_else(|error| {
        log::error!("Failed to stop camera motion before exit: {error}");
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
            // camera_power,
            // autofocus,
            go_to_preset,
            set_preset,
            move_camera,
            zoom,
            stop_motion,
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

    let (events_tx, mut events_rx) = mpsc::unbounded_channel::<CameraEvent>();

    let app = tauri_builder
        .manage(UIState::app_state())
        .manage(PortChangeLock::default())
        .manage(CameraEventSenderState::new(events_tx))
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } => {
                if window.label() == "main" {
                    stop_camera_before_exit(window.app_handle());
                }
            }
            WindowEvent::Destroyed if window.label() == "main" => {
                window.app_handle().exit(0);
            }
            _ => {}
        })
        .setup(move |app| {
            specta_builder.mount_events(app);

            let handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                while let Some(event) = events_rx.recv().await {
                    let _ = publish_camera_event(&handle, event);
                }
            });

            let handle = app.handle();

            if let Err(error) = tauri::async_runtime::block_on(initialize_ui_state(handle)) {
                log::error!("Failed to initialize UI state: {error}");

                let _ = with_ui_emit(handle, |ui| {
                    ui.set_error(&format!("Could not connect to camera: {error}"));
                    Ok(())
                });
            }

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

    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            stop_camera_before_exit(app);
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
