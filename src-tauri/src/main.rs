// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod port_state;
mod visca;

use tauri::utils::assets::EmbeddedAssets;
use tauri::{
    AboutMetadata, Context, CustomMenuItem, Manager, Menu, MenuItem, Submenu, WindowEvent, Wry,
};
use tauri_plugin_window_state::StateFlags;

use crate::error::Result;
use crate::port_state::PortState;
use crate::visca::{Autofocus, Move, Power, Preset, Zoom};

fn send_staus(app_handle: &tauri::AppHandle, status: &str) -> () {
    app_handle.emit_to("main", "status", status).ok();
}

#[tauri::command]
fn camera_power(port_state: tauri::State<PortState>, state: bool) -> Result<()> {
    println!("Power: {}", state);

    port_state.execute(1, Power::from(state))?;

    Ok(())
}

#[tauri::command]
fn autofocus(port_state: tauri::State<PortState>, state: bool) -> Result<()> {
    println!("Autofocus: {}", state);

    port_state.execute(1, Autofocus::from(state))?;

    Ok(())
}

#[tauri::command]
fn go_to_preset(port_state: tauri::State<PortState>, preset: u8) -> Result<()> {
    println!("Go To Preset: {}", preset);

    port_state.execute(1, Preset::Recall(preset))?;

    Ok(())
}

#[tauri::command]
fn set_preset(port_state: tauri::State<PortState>, preset: u8) -> Result<()> {
    println!("Set Preset: {}", preset);

    port_state.execute(1, Preset::Set(preset))?;

    Ok(())
}

#[tauri::command]
fn move_camera(port_state: tauri::State<PortState>, direction: &str) -> Result<()> {
    println!("Direction: {}", direction);

    port_state.execute(
        1,
        match direction {
            "left" => Move::Left(1),
            "right" => Move::Right(1),
            "up" => Move::Up(1),
            "down" => Move::Down(1),
            _ => Move::Stop,
        },
    )?;

    Ok(())
}

#[tauri::command]
fn stop_move(port_state: tauri::State<PortState>) -> Result<()> {
    println!("Stop Move");

    port_state.execute(1, Move::Stop)?;

    Ok(())
}

#[tauri::command]
fn zoom(port_state: tauri::State<PortState>, direction: &str) -> Result<()> {
    println!("Zoom: {}", direction);

    port_state.execute(1, Zoom::from(direction))?;

    Ok(())
}

#[tauri::command]
fn stop_zoom(port_state: tauri::State<PortState>) -> Result<()> {
    println!("Stop Zoom");

    port_state.execute(1, Zoom::Stop)?;

    Ok(())
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
            .updater_target("darwin-universal")
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
            stop_move,
            zoom,
            stop_zoom,
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
                let port_name = event
                    .payload()
                    .map(|payload| serde_json::from_str::<&str>(payload).ok())
                    .flatten();

                send_staus(
                    &app_handle,
                    match port_name {
                        Some(_) => "Connecting",
                        None => "Disconnecting",
                    },
                );

                let port_state = app_handle.state::<PortState>();

                if let Err(error) = port_state.set_port(port_name) {
                    app_handle
                        .emit_all("port-change-error", error.to_string())
                        .unwrap_or(());
                } else {
                    send_staus(
                        &app_handle,
                        match port_name {
                            Some(_) => "Connected",
                            None => "Disconnected",
                        },
                    );
                }
            });

            Ok(())
        })
        .run(context)
        .expect("error while running tauri application")
}
