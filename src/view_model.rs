use slint::{Model, Weak};

use crate::commands::{Commands, Request};
use crate::model::image_container::{FilterState, ImageSettings};
use crossbeam::channel::Sender;
use std::error::Error;
use std::sync::{Arc, Mutex};

slint::include_modules!();

#[derive(Default)]
pub struct AppState {
    pub selected_photo_id: Option<u32>,
    pub number_photos: u32,
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

        v.ui.window().set_maximized(true);
        v.setup_callbacks();

        Ok(v)
    }

    fn send_load_request(&self, offset: i32) {
        let mut state = self.appstate.lock().unwrap();

        // Update the selected photo id by adding 1
        if let Some(selected_photo) = state.selected_photo_id {
            // Prevent from going out of bounds
            if selected_photo + offset as u32 >= state.number_photos {
                return;
            }

            // Update state
            state.selected_photo_id = Some(selected_photo + 1);

            // Ignore the result for now
            let _ = self.sender.send(Commands::LoadPhoto(selected_photo).high());
        }
    }

    fn setup_callbacks(&self) {
        // Next photo callback
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        let weak = self.ui.as_weak();

        self.ui.on_next_photo(move || {
            let mut state = appstate.lock().unwrap();

            // Update the selected photo id by adding 1
            if let Some(selected_photo) = state.selected_photo_id {
                let new_photo_id = selected_photo + 1;

                // Prevent from going out of bounds
                if new_photo_id >= state.number_photos {
                    return;
                }

                // Update state
                state.selected_photo_id = Some(new_photo_id);

                // Ignore the result for now
                let _ = sender.send(Commands::LoadPhoto(new_photo_id).high());

                // Immediately show the already loaded preview
                let _ = weak.upgrade_in_event_loop(move |handle| {
                    let filmstrip = handle.get_filmstrip_images();
                    if (new_photo_id as usize) < filmstrip.row_count() {
                        handle
                            .set_current_image(filmstrip.row_data(new_photo_id as usize).unwrap());
                    }
                });
            }
        });

        // Previous photo callback
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_prev_photo(move || {
            let mut state = appstate.lock().unwrap();

            // Update the selected photo id by adding 1
            if let Some(selected_photo) = state.selected_photo_id {
                let new_photo_id = selected_photo - 1;

                // Prevent from going out of bounds
                if selected_photo <= 0 {
                    return;
                }

                // Update state
                state.selected_photo_id = Some(new_photo_id);

                // Ignore the result for now
                let _ = sender.send(Commands::LoadPhoto(new_photo_id).high());
            }
        });

        // Change image settings
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_settings_changed(move |brightness, contrast| {
            let state = appstate.lock().unwrap();

            let _ = sender.send(
                Commands::AdjustImagesettings(
                    state.selected_photo_id.unwrap(),
                    ImageSettings {
                        brightness,
                        contrast,
                    },
                )
                .into(),
            );
        });

        // Set filters
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_accept_photo(move || {
            let state = appstate.lock().unwrap();
            sender
                .send(
                    Commands::SetNormalFilter(
                        state.selected_photo_id.unwrap(),
                        FilterState::Accepted,
                    )
                    .request(),
                )
                .unwrap();
        });

        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_reject_photo(move || {
            let state = appstate.lock().unwrap();
            sender
                .send(
                    Commands::SetNormalFilter(
                        state.selected_photo_id.unwrap(),
                        FilterState::Rejected,
                    )
                    .request(),
                )
                .unwrap();
        });

        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_scherm_photo(move |flag| {
            let state = appstate.lock().unwrap();
            sender
                .send(Commands::SetSchermFilter(state.selected_photo_id.unwrap(), flag).request())
                .unwrap();
        });
    }

    pub fn get_ui_handle(&self) -> Weak<AppWindow> {
        self.ui.as_weak()
    }

    pub fn get_appstate(&self) -> Arc<Mutex<AppState>> {
        self.appstate.clone()
    }

    pub fn send_model_save(&self) {
        self.sender.send(Commands::SaveState.critical()).unwrap();
    }

    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.ui.run()
    }
}
