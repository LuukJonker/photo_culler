// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod error;
mod model;
mod response_listener;
mod view_model;

use crossbeam::channel::unbounded;
use std::{env::args, error::Error};

use crate::{
    commands::{Commands, Response},
    model::Model,
    response_listener::ResponseListener,
    view_model::ViewModel,
};

fn main() -> Result<(), Box<dyn Error>> {
    // Channel for the responses back to the view model
    let model = Model::new();
    let (response_sender, response_receiver) = unbounded::<Response>();

    // All the objects for the different parts of the program
    let vm = ViewModel::new(model.get_sender())?;
    let listener = ResponseListener::new(
        model.get_sender(),
        response_receiver,
        vm.get_ui_handle(),
        vm.get_appstate(),
    );

    model
        .get_sender()
        .send(Commands::LoadDirectory(args().nth(1).unwrap().into()).request())?;

    // Start the threads
    model.run(response_sender);
    listener.start();

    // Blocks the main thread, call it last
    vm.run()?;

    // Save the model after the view is closed, before the thread is
    // stopped and the model is killed.
    vm.send_model_save();

    // Block until the model is done
    // model.block_for_workers();

    Ok(())
}
