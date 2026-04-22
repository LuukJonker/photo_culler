use slint::{Image, Model, ModelRc, VecModel, Weak};

use crate::{
    commands::{Commands, Request, Response, ResponseData},
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
                        ResponseData::LoadedPhoto(id, buffer, settings) => {
                            // If not good photo, dont show
                            if Some(id) != self.appstate.lock().unwrap().selected_photo_id {
                                continue;
                            }

                            self.ui_handle
                                .upgrade_in_event_loop(move |handle| {
                                    handle.set_brightness_val(settings.brightness);
                                    handle.set_current_image(Image::from_rgb8(buffer))
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
                                    let empty_images: Vec<Image> =
                                        vec![Image::default(); num_images as usize];
                                    let model = std::rc::Rc::new(VecModel::from(empty_images));
                                    handle.set_filmstrip_images(ModelRc::from(model));
                                })
                                .expect("Init filmstrip event loop error");

                            self.sender.send(Commands::LoadPhoto(0).high()).unwrap();

                            for i in 0..num_images {
                                self.sender.send(Commands::LoadThumbnail(i).low()).unwrap();
                            }
                        }

                        ResponseData::SettingsForPhoto(id, settings) => {
                            // Check if the settings are actually for the current image, or that we even have a current image
                            if Some(id) != self.appstate.lock().unwrap().selected_photo_id {
                                // Continue to next message if the current image is not the one we got the settings for
                                continue;
                            }

                            let _ = self.ui_handle.upgrade_in_event_loop(move |handle| {
                                handle.set_brightness_val(settings.brightness);
                                handle.set_contrast_val(settings.contrast);
                            });
                        }

                        ResponseData::LoadedPreview(buffer) => {
                            if let Commands::LoadThumbnail(index) = msg.request.command() {
                                self.ui_handle
                                    .upgrade_in_event_loop(move |handle| {
                                        handle
                                            .get_filmstrip_images()
                                            .set_row_data(index as usize, Image::from_rgb8(buffer));
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
