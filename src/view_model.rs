use slint::{Model, Weak};
use tracing::field::debug;
use tracing::{Level, debug, debug_span, info, span};

use crate::commands::{Commands, Request};
use crate::model::image_container::{FilterState, ImageContainerState, ImageSettings};
use crossbeam::channel::Sender;
use std::error::Error;
use std::sync::{Arc, Mutex};

slint::include_modules!();

/// Filter for the filmstrip view.
#[derive(Default)]
pub enum FilmstripFilter {
    /// Show all images.
    #[default]
    All,
    /// Show only accepted images.
    Accepted,

    /// Show accepted and unknown images.
    NotRejected,
    /// Show only rejected images.
    Rejected,
}

impl FilmstripFilter {
    /// Returns true if the given filter state should be included in this view filter.
    pub fn included(&self, filter_settings: &FilterState) -> bool {
        match self {
            Self::All => true,
            Self::Accepted => matches!(filter_settings, FilterState::Accepted),
            Self::Rejected => matches!(filter_settings, FilterState::Rejected),
            Self::NotRejected => {
                matches!(filter_settings, FilterState::Unknown)
                    || matches!(filter_settings, FilterState::Accepted)
            }
        }
    }
}

/// The main application state tracked on the Rust side.
#[derive(Default)]
pub struct AppState {
    /// ID of the currently selected photo.
    pub current_photo_id: Option<u32>,
    /// Total number of photos in the current directory.
    pub num_photos: u32,
    /// State of each image container (metadata, settings, etc.).
    pub photos_infos: Vec<ImageContainerState>,

    /// Indices of images that are currently visible based on the filter.
    pub view_images_mapping: Vec<u32>,

    /// Index within `view_images_indices` of the currently selected image.
    pub current_view_index: Option<usize>,

    /// The active view filter.
    filter: FilmstripFilter,
}

impl AppState {
    /// Updates the view filter and returns the new set of visible image indices.
    /// NOTE: Update the selected photo id in slint from the appstate after using this function.
    fn set_filter(&mut self, filter: FilmstripFilter) -> Vec<u32> {
        self.view_images_mapping.clear();

        for (i, photo_info) in self.photos_infos.iter().enumerate() {
            if self.current_photo_id == Some(i as u32) {
                // TODO: Make it the the first left photo instead of right
                self.current_view_index = Some(self.view_images_mapping.len());
                dbg!(i, self.current_view_index);
            }

            if filter.included(&photo_info.filter_settings.filter) {
                self.view_images_mapping.push(i as u32);
            }
        }

        // Fix if this and every photo after got filtered out
        if self.current_view_index >= Some(self.view_images_mapping.len()) {
            self.current_view_index = Some(self.view_images_mapping.len() - 1);
        }

        // Update id
        self.current_photo_id = Some(
            *self
                .view_images_mapping
                .get(self.current_view_index.unwrap())
                .unwrap(),
        );

        self.filter = filter;

        self.view_images_mapping.clone()
    }

    /// Updates the filter state for a specific image and adjusts the view if necessary.
    fn set_image_filter(&mut self, index: usize, filter_settings: FilterState) {
        if !self.filter.included(&filter_settings) {
            if let Some(view_index) = self.current_view_index
                && let Some(current_id) = self.current_photo_id
                && self.view_images_mapping[view_index] == current_id
            {
                self.view_images_mapping.remove(view_index);
            }
            // else we probably have to find it, but more likely a bug so...
            panic!("Setting filter on not current image");
        }

        self.photos_infos[index].filter_settings.filter = filter_settings;
    }
}

/// The View Model that bridges the Slint UI and the Rust backend.
pub struct ViewModel {
    /// The Slint UI handle.
    ui: AppWindow,

    /// Sender for dispatching requests to worker threads.
    sender: Sender<Request>,

    /// Thread-safe application state.
    appstate: Arc<Mutex<AppState>>,
}

impl ViewModel {
    /// Creates a new ViewModel and initializes the UI.
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

    /// Connects Slint UI callbacks to Rust logic.
    fn setup_callbacks(&self) {
        // Helper to change the selected photo
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();
        let weak = self.ui.as_weak();

        let go_to_photo = move |index: usize| {
            let mut state = appstate.lock().unwrap();

            // Prevent from going out of bounds
            if index >= state.view_images_mapping.len() {
                return;
            }

            let new_photo_id = state.view_images_mapping[index];

            // Update state
            state.current_photo_id = Some(new_photo_id);

            // Ignore the result for now
            let _ = sender.send(Commands::LoadPhoto(new_photo_id).high());

            weak.upgrade_in_event_loop(move |handle| {
                // Immediately show the already loaded preview if available in filmstrip
                let filmstrip = handle.get_filmstrip_images();
                if (new_photo_id as usize) < filmstrip.row_count() {
                    let preview = filmstrip.row_data(new_photo_id as usize).unwrap();
                    handle.set_current_image(preview);
                }

                // Update the filmstrip to show the selected photo
                handle.set_selected_index(new_photo_id as i32);
            })
            .unwrap();
        };

        // Next photo callback
        let go_to = go_to_photo.clone();
        let appstate = self.appstate.clone();
        self.ui.on_next_photo(move || {
            let state = appstate.lock().unwrap();

            if let Some(id) = state.current_photo_id {
                let current_index = match state.view_images_mapping.iter().position(|i| *i == id) {
                    Some(v) => v,
                    None => return,
                };

                if current_index >= state.view_images_mapping.len() {
                    return;
                }

                // FIXME: Explicitly drop mutex lock bc otherwise it fails in go_to
                drop(state);

                go_to(current_index + 1);
            }
        });

        // Previous photo callback
        let go_to = go_to_photo.clone();
        let appstate = self.appstate.clone();
        self.ui.on_prev_photo(move || {
            let state = appstate.lock().unwrap();

            if let Some(id) = state.current_photo_id {
                let current_index = match state.view_images_mapping.iter().position(|i| *i == id) {
                    Some(v) => v,
                    None => return,
                };

                drop(state);

                if current_index == 0 {
                    return;
                }

                go_to(current_index - 1);
            }
        });

        // Select photo callback
        let go_to = go_to_photo.clone();
        self.ui.on_select_photo(move |index| {
            debug!("on_select_photo(index={})", index);

            go_to(index as usize);
        });

        // Load raw callback
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_load_raw(move || {
            debug!("on_load_raw()");

            let state = appstate.lock().unwrap();

            // Update the selected photo id by adding 1
            if let Some(selected_photo) = state.current_photo_id {
                // Ignore the result for now
                let _ = sender.send(Commands::LoadRawPhoto(selected_photo).high());
            }
        });

        // Change image settings
        let sender = self.sender.clone();
        let appstate = self.appstate.clone();

        self.ui.on_settings_changed(move |brightness, contrast| {
            debug!(
                "on_settings_changed brightness: {} contrast: {}",
                brightness, contrast
            );
            let state = appstate.lock().unwrap();

            if let Some(selected_photo_id) = state.current_photo_id {
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
            span!(Level::DEBUG, "on_accept_photo");

            let mut state = appstate.lock().unwrap();
            if let Some(selected_photo_id) = state.current_photo_id {
                debug!(photo_id = selected_photo_id);
                state.set_image_filter(selected_photo_id as usize, FilterState::Accepted);

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
            debug_span!("on_reject_photo");

            // BUG: Crashes when this filter settings change means photo is out of the view
            let mut state = appstate.lock().unwrap();
            if let Some(selected_photo_id) = state.current_photo_id {
                debug!(photo_id = selected_photo_id);
                state.set_image_filter(selected_photo_id as usize, FilterState::Rejected);

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

            if let Some(selected_photo_id) = state.current_photo_id {
                sender
                    .send(Commands::SetSchermFilter(selected_photo_id, flag).request())
                    .unwrap();
            }
        });

        let state = self.appstate.clone();
        let weak = self.get_ui_handle();

        self.ui.on_set_view_filter(move |filter_id| {
            let filter = match filter_id {
                0 => FilmstripFilter::All,
                1 => FilmstripFilter::Accepted,
                2 => FilmstripFilter::Rejected,
                _ => {
                    eprintln!("[BUG] Wrong filter id was given");
                    FilmstripFilter::All
                }
            };

            let mut state = state.lock().unwrap();

            // Need to update the select photo id after using this function
            let view_images_indices = state.set_filter(filter);
            let current_id = state.current_photo_id.unwrap();

            weak.upgrade_in_event_loop(move |handle| {
                let visible_indices_model = std::rc::Rc::new(slint::VecModel::from_iter(
                    view_images_indices.iter().map(|u| *u as i32),
                ));

                handle.set_filmstrip_visible_indices(slint::ModelRc::from(visible_indices_model));

                handle.set_selected_index(current_id as i32);
            })
            .unwrap();
        });

        let state = self.appstate.clone();
        let sender = self.sender.clone();
        self.ui.on_start_export(move |dir| {
            for (i, image_info) in state.lock().unwrap().photos_infos.iter().enumerate() {
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
                weak.upgrade_in_event_loop(move |handle| {
                    handle
                        .set_export_output_directory(folder.to_string_lossy().into_owned().into());
                })
                .unwrap();
            }
        });

        let sender = self.sender.clone();
        self.ui.on_browse_photos_dir(move || {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                sender.send(Commands::LoadDirectory(folder).high()).unwrap();
            }
        })
    }

    pub fn get_ui_handle(&self) -> Weak<AppWindow> {
        self.ui.as_weak()
    }

    pub fn get_appstate(&self) -> Arc<Mutex<AppState>> {
        self.appstate.clone()
    }

    pub fn send_model_save(&self) {
        self.sender.send(Commands::SaveState.critical()).unwrap();
        self.sender.send(Commands::KillThread.high()).unwrap();
    }

    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.ui.run()
    }
}
