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

    fn setup_callbacks(&self) {
        // Helper to change the selected photo
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();
        let weak = self.ui.as_weak();

        let go_to_photo = move |new_photo_id: u32| {
            let mut state = appstate.lock().unwrap();

            // Prevent from going out of bounds
            if new_photo_id >= state.number_photos {
                return;
            }

            // Update state
            state.selected_photo_id = Some(new_photo_id);

            // Ignore the result for now
            let _ = sender.send(Commands::LoadPhoto(new_photo_id).high());

            let _ = weak.upgrade_in_event_loop(move |handle| {
                // Immediately show the already loaded preview if available in filmstrip
                let filmstrip = handle.get_filmstrip_images();
                if (new_photo_id as usize) < filmstrip.row_count() {
                    let preview = filmstrip.row_data(new_photo_id as usize).unwrap();
                    // Only set if it's not the default (empty) image
                    // Note: Slint images don't have an easy "is_empty" but we can check if it has a size if we really wanted to.
                    // For now, we just set it.
                    handle.set_current_image(preview);
                }

                // Update the filmstrip to show the selected photo
                handle.set_selected_index(new_photo_id as i32);
            });
        };

        // Next photo callback
        let go_to = go_to_photo.clone();
        let appstate = self.appstate.clone();
        self.ui.on_next_photo(move || {
            let selected_photo = appstate.lock().unwrap().selected_photo_id;
            if let Some(id) = selected_photo {
                go_to(id + 1);
            }
        });

        // Previous photo callback
        let go_to = go_to_photo.clone();
        let appstate = self.appstate.clone();
        self.ui.on_prev_photo(move || {
            let selected_photo = appstate.lock().unwrap().selected_photo_id;
            if let Some(id) = selected_photo {
                if id > 0 {
                    go_to(id - 1);
                }
            }
        });

        // Select photo callback
        let go_to = go_to_photo.clone();
        self.ui.on_select_photo(move |index| {
            go_to(index as u32);
        });

        // Load raw callback
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_load_raw(move || {
            let state = appstate.lock().unwrap();

            // Update the selected photo id by adding 1
            if let Some(selected_photo) = state.selected_photo_id {
                // Ignore the result for now
                let _ = sender.send(Commands::LoadRawPhoto(selected_photo).high());
            }
        });

        // Change image settings
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_settings_changed(move |brightness, contrast| {
            let state = appstate.lock().unwrap();

            if let Some(selected_photo_id) = state.selected_photo_id {
                let _ = sender.send(
                    Commands::AdjustImagesettings(
                        selected_photo_id,
                        ImageSettings {
                            brightness,
                            contrast,
                        },
                    )
                    .into(),
                );
            }
        });

        // Set filters
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();
        let weak = self.ui.as_weak();

        self.ui.on_accept_photo(move || {
            let state = appstate.lock().unwrap();
            if let Some(selected_photo_id) = state.selected_photo_id {
                weak.upgrade_in_event_loop(move |handle| {
                    handle
                        .get_filmstrip_statuses()
                        .set_row_data(selected_photo_id as usize, 1)
                })
                .unwrap();

                sender
                    .send(
                        Commands::SetNormalFilter(selected_photo_id, FilterState::Accepted).request(),
                    )
                    .unwrap();
            }
        });

        let sender = self.sender.clone();
        let appstate = self.appstate.clone();
        let weak = self.ui.as_weak();

        self.ui.on_reject_photo(move || {
            let state = appstate.lock().unwrap();
            if let Some(selected_photo_id) = state.selected_photo_id {
                weak.upgrade_in_event_loop(move |handle| {
                    handle
                        .get_filmstrip_statuses()
                        .set_row_data(selected_photo_id as usize, 2)
                })
                .unwrap();

                sender
                    .send(
                        Commands::SetNormalFilter(selected_photo_id, FilterState::Rejected).request(),
                    )
                    .unwrap();
            }
        });

        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_scherm_photo(move |flag| {
            let state = appstate.lock().unwrap();
            if let Some(selected_photo_id) = state.selected_photo_id {
                sender
                    .send(Commands::SetSchermFilter(selected_photo_id, flag).request())
                    .unwrap();
            }
        });

        let sender = self.sender.clone();
        let appstate = self.appstate.clone();
        let weak = self.ui.as_weak();

        self.ui.on_clear_filter(move || {
            let state = appstate.lock().unwrap();
            if let Some(selected_photo_id) = state.selected_photo_id {
                weak.upgrade_in_event_loop(move |handle| {
                    handle
                        .get_filmstrip_statuses()
                        .set_row_data(selected_photo_id as usize, 0)
                })
                .unwrap();

                sender
                    .send(
                        Commands::SetNormalFilter(selected_photo_id, FilterState::Unknown).request(),
                    )
                    .unwrap();
            }
        });

        self.ui.on_set_view_filter(move |filter| {
            println!("View filter changed to: {}", filter);
            // TODO: Implement actual filtering logic in the model
        });

        self.ui.on_start_export(move |dir| {
            println!("Exporting to: {}", dir);
            // TODO: Implement actual export logic
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
