use slint::{Image, Weak};

use crate::{
    commands::{Response, ResponseData},
    view_model::{AppState, AppWindow},
};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ResponseListener {
    ui_handle: Weak<AppWindow>,

    receiver: Receiver<Response>,

    appstate: Arc<Mutex<AppState>>,
}

impl ResponseListener {
    pub fn new(
        receiver: Receiver<Response>,
        ui_handle: Weak<AppWindow>,
        appstate: Arc<Mutex<AppState>>,
    ) -> Self {
        Self {
            ui_handle,
            receiver,
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

                        ResponseData::LoadedDirectory => {
                            self.appstate.lock().unwrap().selected_photo_id = Some(0);
                        }

                        ResponseData::SettingsForPhoto(id, settings) => {
                            // Check if the settings are actually for the current image, or that we even have a current image
                            if Some(id) != self.appstate.lock().unwrap().selected_photo_id {
                                return;
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
