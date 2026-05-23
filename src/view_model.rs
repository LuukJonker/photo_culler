use slint::{Model, Weak};

use crate::commands::{Commands, Request};
use crate::model::image_container::{FilterState, ImageContainerState, ImageSettings};
use crossbeam::channel::Sender;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

slint::include_modules!();

pub enum FilmstripFilter {
    ShowAll,
    ShowAccepted,
    ShowRejected,
}

impl FilmstripFilter {
    pub fn included(&self, filter_settings: &FilterState) -> bool {
        match self {
            Self::ShowAll => true,
            Self::ShowAccepted => matches!(filter_settings, FilterState::Accepted),
            Self::ShowRejected => matches!(filter_settings, FilterState::Rejected),
        }
    }
}

impl Default for FilmstripFilter {
    fn default() -> Self {
        Self::ShowAll
    }
}

#[derive(Default)]
pub struct AppState {
    pub selected_photo_id: Option<u32>,
    pub number_photos: u32,
    pub image_states: Vec<ImageContainerState>,
    pub filter: FilmstripFilter,
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
                        Commands::SetNormalFilter(selected_photo_id, FilterState::Accepted)
                            .request(),
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
                        Commands::SetNormalFilter(selected_photo_id, FilterState::Rejected)
                            .request(),
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
                        Commands::SetNormalFilter(selected_photo_id, FilterState::Unknown)
                            .request(),
                    )
                    .unwrap();
            }
        });

        let state = self.appstate.clone();
        let weak = self.get_ui_handle();

        self.ui.on_set_view_filter(move |filter_id| {
            let filter = match filter_id {
                0 => FilmstripFilter::ShowAll,
                1 => FilmstripFilter::ShowAccepted,
                2 => FilmstripFilter::ShowRejected,
                _ => {
                    eprintln!("[BUG] Wrong filter id was given");
                    FilmstripFilter::ShowAll
                }
            };

            let mut state = state.lock().unwrap();
            println!("{:?}", state.image_states[0]);
            let is_shown: Vec<bool> = state
                .image_states
                .iter()
                .map(|container| filter.included(&container.filter_settings.filter))
                .collect();

            let mut visible_indices = Vec::new();
            let mut current_visible_idx = 0;
            for shown in &is_shown {
                if *shown {
                    visible_indices.push(current_visible_idx);
                    current_visible_idx += 1;
                } else {
                    visible_indices.push(-1);
                }
            }

            weak.upgrade_in_event_loop(move |handle| {
                let is_shown_model = handle.get_filmstrip_is_shown();
                for (i, shown) in is_shown.iter().enumerate() {
                    is_shown_model.set_row_data(i, *shown);
                }

                let visible_indices_model =
                    std::rc::Rc::new(slint::VecModel::from(visible_indices));
                handle.set_filmstrip_visible_indices(slint::ModelRc::from(visible_indices_model));
            })
            .unwrap();

            state.filter = filter;
        });

        let state = self.appstate.clone();
        let sender = self.sender.clone();

        self.ui.on_start_export(move |dir| {
            for (i, image_info) in state.lock().unwrap().image_states.iter().enumerate() {
                if matches!(image_info.filter_settings.filter, FilterState::Accepted) {
                    sender
                        .send(Commands::ExportImage(i as u32, dir.to_string().into()).request())
                        .unwrap();
                }
            }
        });

        let weak = self.ui.as_weak();
        self.ui.on_browse_export_dir(move || {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let _ = weak.upgrade_in_event_loop(move |handle| {
                    handle.set_export_output_directory(folder.to_string_lossy().into_owned().into());
                });
            }
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
