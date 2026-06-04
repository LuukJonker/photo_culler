// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod constants;
mod error;
mod model;
mod response_listener;
mod view_model;

use crossbeam::channel::unbounded;
use std::{env::args, error::Error, thread};
use tracing::{Level, debug, info};
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_unwrap::{OptionExt, ResultExt};

use crate::{
    commands::{Commands, Response},
    constants::{APP_AUTHOR, APP_NAME},
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
    let logs_dir = appdirs::user_log_dir(Some(APP_NAME), Some(APP_AUTHOR));
    if let Ok(logs_dir) = logs_dir {
        if std::fs::create_dir_all(&logs_dir).is_ok() {
            let file_appender = tracing_appender::rolling::daily(&logs_dir, "app.log");
            let (non_blocking, _) = tracing_appender::non_blocking(file_appender);
            let combined_writer = std::io::stdout.and(non_blocking);

            tracing_subscriber::fmt()
                .with_writer(combined_writer)
                .with_max_level(Level::DEBUG)
                .with_thread_names(true)
                .with_thread_ids(true)
                .init();

            info!("Writing logs to {:?}", logs_dir);
        } else {
            tracing_subscriber::fmt()
                .with_max_level(Level::DEBUG)
                .with_thread_names(true)
                .with_thread_ids(true)
                .init();
            tracing::warn!(
                "Failed to create log directory at {:?}. Falling back to console logging only.",
                logs_dir
            );
        }
    } else {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .with_thread_names(true)
            .with_thread_ids(true)
            .init();
    }

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

    if let Some(folder_path) = args().nth(1) {
        model
            .get_sender()
            .send(Commands::LoadDirectory(folder_path.into()).request())?;
    }

    // Start the threads
    let mut worker_handles = model.run(response_sender);
    let listener_handle = listener.start();

    // Blocks the main thread, call it last
    vm.run()?;

    // Save the model after the view is closed, before the thread is
    // stopped and the model is killed.
    vm.send_model_save();

    //Block until the model is done
    for i in 0..10 {
        worker_handles.pop().unwrap_or_log();
        debug!("Closed handle {}", i);
    }
    listener_handle.join().unwrap_or_log();

    Ok(())
}
