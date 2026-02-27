use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

mod image_browser;
pub mod image_container;

use crate::commands::{Commands, Request, Response, ResponseData};
use crate::error::ModelError;
use crate::model::image_browser::ImageBrowser;

pub struct Model {
    sender: Sender<Request>,
    receiver: Receiver<Request>,

    response_sender: Sender<Response>,

    browser: Option<ImageBrowser>,
}

impl Model {
    pub fn new(response_sender: Sender<Response>) -> Model {
        let (sender, receiver) = channel::<Request>();

        Model {
            sender,
            receiver,
            response_sender,
            browser: None,
        }
    }

    pub fn get_sender_inst(&self) -> Sender<Request> {
        self.sender.clone()
    }

    fn load_photo(&self, index: u32) -> Result<ResponseData, ModelError> {
        let browser = self
            .browser
            .as_ref()
            .ok_or(ModelError::PhotoNotFound)?;

        let image = browser.preview_at_index(index as usize);
        let settings = browser.settings_at_index(index as usize);

        Ok(ResponseData::LoadedPhoto(image, settings))
    }

    fn event_loop(mut self) {
        for msg in &self.receiver {
            let cmd = &msg.command;

            let return_value = match cmd {
                Commands::LoadPhoto(id) => self.load_photo(*id),

                Commands::LoadDirectory(path) => {
                    self.browser = Some(ImageBrowser::new(path.into()));

                    self.load_photo(0)
                },

                Commands::AdjustImagesettings(id, settings) => {
                    self.browser.as_mut().unwrap().set_imagesettings(*id as usize, settings.clone());

                    // Nothing to return
                    continue;
                },

                Commands::LoadFilmstrip => {
                    todo!()
                }
            };

            self.response_sender.send(Response {request: msg, value: return_value}).unwrap();
        }
    }

    pub fn run(self) {
        thread::spawn(move || self.event_loop());
    }
}
