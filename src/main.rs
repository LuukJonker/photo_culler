// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod model;
mod response_listener;
mod view_model;
mod error;

use std::{error::Error, sync::mpsc::channel};

use crate::{
    commands::{Commands, Response},
    model::Model,
    response_listener::ResponseListener,
    view_model::ViewModel,
};

fn main() -> Result<(), Box<dyn Error>> {
    // Channel for the responses back to the view model
    let (response_sender, response_receiver) = channel::<Response>();

    // All the objects for the different parts of the program
    let model = Model::new(response_sender);
    let vm = ViewModel::new(model.get_sender_inst())?;
    let listener = ResponseListener::new(model.get_sender_inst(), response_receiver, vm.get_ui_handle(), vm.get_appstate());

    model.get_sender_inst().send(Commands::LoadDirectory(
        "/home/luuk/Pictures/Screenshots/".into(),
    ).request())?;

    // Start the threads
    model.run();
    listener.start();
    vm.run()?; // Blocks the main thread

    Ok(())
}
