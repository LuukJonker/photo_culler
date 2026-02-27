use slint::{Image, Model, ModelRc, VecModel, Weak};

use crate::{
    commands::{Commands, Request, Response, ResponseData},
    view_model::{AppState, AppWindow},
};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ResponseListener {
    ui_handle: Weak<AppWindow>,

    sender: Sender<Request>,
    receiver: Receiver<Response>,

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
                        ResponseData::LoadedPhoto(buffer, settings) => {
                            self.ui_handle
                                .upgrade_in_event_loop(move |handle| {
                                    handle.set_brightness_val(settings.brightness);
                                    handle.set_current_image(Image::from_rgb8(buffer))
                                })
                                .expect("Load photo event loop error");
                        }

                        ResponseData::LoadedDirectory(num_images) => {
                            self.appstate.lock().unwrap().selected_photo_id = Some(0);

                            // Initialize the filmstrip with empty placeholder images
                            self.ui_handle
                                .upgrade_in_event_loop(move |handle| {
                                    let empty_images: Vec<Image> =
                                        vec![Image::default(); num_images as usize];
                                    let model = std::rc::Rc::new(VecModel::from(empty_images));
                                    handle.set_filmstrip_images(ModelRc::from(model));
                                })
                                .expect("Init filmstrip event loop error");

                            self.sender.send(Commands::LoadPhoto(0).request()).unwrap();

                            for i in 0..num_images {
                                self.sender
                                    .send(Commands::LoadThumbnail(i).request())
                                    .unwrap();
                            }
                        }

                        ResponseData::SettingsForPhoto(id, settings) => {
                            // Check if the settings are actually for the current image, or that we even have a current image
                            if Some(id) != self.appstate.lock().unwrap().selected_photo_id {
                                // Continue to next message if the current image is not the one we got the settings for
                                continue;
                            }
                        }

                        ResponseData::LoadedPreview(buffer) => {
                            if let Commands::LoadThumbnail(index) = msg.request.command {
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
