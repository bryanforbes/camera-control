// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate log;
extern crate pretty_env_logger;

mod error;
mod port_state;

use crate::error::Result;

use log::debug;
use pelcodrs::{AutoCtrl, Direction, Message, MessageBuilder, Speed};
use port_state::{PortState, with_port};
use tauri::{
    Manager, WindowEvent,
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tauri_plugin_window_state::StateFlags;

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
async fn open_settings(app_handle: tauri::AppHandle) -> Result<()> {
    open_settings_window(&app_handle)
}

#[tauri::command]
fn ready(
    app_handle: tauri::AppHandle,
    window: tauri::Window,
    port_state: tauri::State<PortState>,
) -> Result<()> {
    with_port(port_state, |port| port.emit_to(&app_handle, window.label()))
}

#[tauri::command]
fn set_port(
    port_name: Option<&str>,
    handle: tauri::AppHandle,
    port_state: tauri::State<PortState>,
) -> Result<()> {
    debug!("Port name: {port_name:?}");

    with_port(port_state, |port| port.set(handle, port_name))
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

    #[allow(unused_mut)]
    let mut updater = tauri_plugin_updater::Builder::new();

    #[cfg(target_os = "macos")]
    {
        updater = updater.target("darwin-universal");
    }

    tauri::Builder::default()
        .plugin(updater.build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(PortState::default())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .build(),
        )
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
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    std::process::exit(0);
                }
            }
        })
        .setup(|app| {
            let port_state = app.state::<PortState>();

            with_port(port_state, |port| port.initialize(app.handle()))?;

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
                        println!("downloaded {downloaded} from {content_length:?}");
                    },
                    || {
                        println!("download finished");
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
