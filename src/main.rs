// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod constants;
mod error;
mod model;
mod response_listener;
mod view_model;

use crossbeam::channel::unbounded;
use std::{env::args, error::Error, fmt::Debug, thread};
use tracing::info;

use crate::{
    commands::{Commands, Response},
    model::Model,
    response_listener::ResponseListener,
    view_model::ViewModel,
};

/// The main entry point of the Photo Culler application.
///
/// It initializes the model, view model, and response listener,
/// processes command line arguments for the initial directory,
/// and starts the main application loop.
fn main() -> Result<(), Box<dyn Error>> {
    // Setup logging
    let file_appender = tracing_appender::rolling::daily("logs", "app.log");
    let (non_blocking, _) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt().with_writer(non_blocking).init();

    info!("Application starting...");

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

    model.get_sender().send(
        Commands::LoadDirectory(
            args()
                .nth(1)
                .ok_or_else(|| "No directory specified".to_string())?,
        )
        .request(),
    )?;

    // Start the threads
    let worker_handles = model.run(response_sender);
    listener.start();

    // Blocks the main thread, call it last
    vm.run()?;

    // Save the model after the view is closed, before the thread is
    // stopped and the model is killed.
    vm.send_model_save();

    // Block until the model is done
    // for handle in worker_handles {
    //     handle.join().unwrap();
    // }
    //
    // For now just a simple timer
    thread::sleep_ms(2000);

    Ok(())
}
