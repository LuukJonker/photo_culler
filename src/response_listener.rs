use slint::{Image, Model, ModelRc, VecModel, Weak};

use crate::{
    commands::{Commands, Request, Response, ResponseData},
    model::image_container::FilterState,
    view_model::{AppState, AppWindow},
};
use crossbeam::channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ResponseListener {
    // Sender to send requests to the model
    sender: Sender<Request>,

    // Receiver to receive responses from the workers
    receiver: Receiver<Response>,

    // Weak handle of the ui to allow for pushing updates to the ui
    ui_handle: Weak<AppWindow>,

    // The frontend state of the app
    appstate: Arc<Mutex<AppState>>,
}

impl ResponseListener {
    pub fn new(
        sender: Sender<Request>,
        receiver: Receiver<Response>,
        ui_handle: Weak<AppWindow>,
        appstate: Arc<Mutex<AppState>>,
    ) -> Self {
        Self {
            sender,
            receiver,
            ui_handle,
            appstate,
        }
    }

    pub fn start(self) {
        thread::spawn(move || {
            for msg in self.receiver.iter() {
                let value = msg.value;

                if let Ok(data) = value {
                    match data {
                        ResponseData::LoadedPhoto(id, buffer, state) => {
                            // If not good photo, dont show
                            if Some(id) != self.appstate.lock().unwrap().selected_photo_id {
                                continue;
                            }

                            let filename = state.path.file_name().unwrap_or_default().to_string_lossy().to_string();

                            self.ui_handle
                                .upgrade_in_event_loop(move |handle| {
                                    handle.set_brightness_val(state.image_settings.brightness);
                                    handle.set_contrast_val(state.image_settings.contrast);
                                    handle.set_is_scherm(state.filter_settings.scherm);
                                    handle.set_selected_photo_filename(filename.into());
                                    handle.set_current_image(Image::from_rgb8(buffer));
                                })
                                .expect("Load photo event loop error");
                        }

                        ResponseData::LoadedDirectory(num_images) => {
                            // Update the state
                            let mut state = self.appstate.lock().unwrap();
                            state.selected_photo_id = Some(0);
                            state.number_photos = num_images;

                            // Initialize the filmstrip with empty placeholder images
                            self.ui_handle
                                .upgrade_in_event_loop(move |handle| {
                                    handle.set_photo_count(num_images as i32);

                                    let empty_images: Vec<Image> =
                                        vec![Image::default(); num_images as usize];
                                    let model = std::rc::Rc::new(VecModel::from(empty_images));
                                    handle.set_filmstrip_images(ModelRc::from(model));

                                    let empty_statuses: Vec<i32> = vec![0; num_images as usize];
                                    let status_model =
                                        std::rc::Rc::new(VecModel::from(empty_statuses));
                                    handle.set_filmstrip_statuses(ModelRc::from(status_model));
                                })
                                .expect("Init filmstrip event loop error");

                            self.sender.send(Commands::LoadPhoto(0).high()).unwrap();

                            for i in 0..num_images {
                                self.sender.send(Commands::LoadThumbnail(i).low()).unwrap();
                            }
                        }

                        ResponseData::LoadedPreview(buffer, filter_settings) => {
                            if let Commands::LoadThumbnail(index) = msg.request.command() {
                                self.ui_handle
                                    .upgrade_in_event_loop(move |handle| {
                                        handle
                                            .get_filmstrip_images()
                                            .set_row_data(index as usize, Image::from_rgb8(buffer));

                                        let state_repr = match filter_settings.filter {
                                            FilterState::Accepted => 1,
                                            FilterState::Rejected => 2,
                                            FilterState::Unknown => 0,
                                        };

                                        handle
                                            .get_filmstrip_statuses()
                                            .set_row_data(index as usize, state_repr);
                                    })
                                    .expect("Load preview event loop error");
                            }
                        }
                    }
                } else if let Err(e) = value {
                    // Error occured
                    println!("MODEL ERROR: {}", e);
                }
            }
        });
    }
}
