use slint::Weak;

use crate::commands::{Request, Commands};
use crate::model::image_container::ImageSettings;
use std::sync::{Arc, Mutex};
use std::{error::Error, sync::mpsc::Sender};

slint::include_modules!();

#[derive(Default)]
pub struct AppState {
    pub selected_photo_id: Option<u32>,
}

pub struct ViewModel {
    ui: AppWindow,

    // Connection to the worker threads
    sender: Sender<Request>,

    // App state that has to be kept track of on the rust side
    appstate: Arc<Mutex<AppState>>,
}

impl ViewModel {
    pub fn new(sender: Sender<Request>) -> Result<ViewModel, Box<dyn Error>> {
        let v = ViewModel {
            ui: AppWindow::new()?,
            sender,
            appstate: Arc::new(Mutex::new(AppState::default())),
        };

        v.appstate.lock().unwrap().selected_photo_id = Some(0);

        v.setup_callbacks();

        Ok(v)
    }

    fn setup_callbacks(&self) {
        // Next photo callback
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_next_photo(move || {
            let mut state = appstate.lock().unwrap();

            // Update the selected photo id by adding 1
            state.selected_photo_id = Some(
                state
                    .selected_photo_id
                    .expect("Callback shouldn't be called if not selected")
                    + 1,
            );

            // Ignore the result for now
            let _ = sender.send(Commands::LoadPhoto(state.selected_photo_id.unwrap()).request());
        });

        // Previous photo callback
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_prev_photo(move || {
            let mut state = appstate.lock().unwrap();

            state.selected_photo_id = Some(
                state
                    .selected_photo_id
                    .expect("Callback shouldn't be called if not selected")
                    - 1,
            );

            let _ = sender.send(Commands::LoadPhoto(state.selected_photo_id.unwrap()).request());
        });

        // Change image settings
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_settings_changed(move |brightness, contrast| {
            let state = appstate.lock().unwrap();

            let _ = sender.send(Commands::AdjustImagesettings(state.selected_photo_id.unwrap(), ImageSettings {brightness, contrast}).request());
        });
    }

    pub fn get_ui_handle(&self) -> Weak<AppWindow> {
        self.ui.as_weak()
    }

    pub fn get_appstate(&self) -> Arc<Mutex<AppState>> {
        self.appstate.clone()
    }

    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.ui.run()
    }
}
