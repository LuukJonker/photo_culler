use crossbeam::channel::{Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};

use crate::commands::{Commands, Request, Response, ResponseData};
use crate::error::ModelError;
use crate::model::ModelState;
use crate::model::image_browser::ImageBrowser;

enum ResponseAction<T> {
    Respond(T),
    Nothing,
    Exit,
}

pub struct Worker {
    receiver: Receiver<Request>,

    response_sender: Sender<Response>,

    browser: Arc<RwLock<Option<ImageBrowser>>>,
}

impl Worker {
    pub fn new(
        receiver: Receiver<Request>,
        response_sender: Sender<Response>,
        state: &ModelState,
    ) -> Self {
        Self {
            receiver,
            response_sender,
            browser: state.browser.clone(),
        }
    }

    fn load_photo(&self, index: u32) -> Result<ResponseData, ModelError> {
        let browser_lock = self.browser.read().unwrap();
        let browser = browser_lock.as_ref().unwrap();

        let container = browser.at_index(index as usize);
        let image = container.get_full_preview()?;
        
        Ok(ResponseData::LoadedPhoto(index, image, container.get_state()))
    }

    fn load_raw_photo(&self, index: u32) -> Result<ResponseData, ModelError> {
        // Get the browser
        let browser_lock = self.browser.read().unwrap();
        let browser = browser_lock.as_ref().unwrap();

        // Get a container
        let container = browser.at_index(index as usize);
        let image = container.get_full_preview()?;

        Ok(ResponseData::LoadedPhoto(index, image, container.get_state()))
    }

    fn load_preview(&self, index: u32) -> Result<ResponseData, ModelError> {
        let browser_lock = self.browser.read().unwrap();
        let browser = browser_lock.as_ref().ok_or(ModelError::DirectoryNotFound)?;

        let container = browser.at_index(index as usize);
        let image = container.get_thumbnail()?;
        let filter = container.filter();

        Ok(ResponseData::LoadedPreview(image, filter))
    }

    fn load_directory(&mut self, path: String) -> Result<ResponseData, ModelError> {
        // Don't check if the current browser is the same as we are trying to load.
        // User might want to reload the browser.
        let browser = ImageBrowser::new(path.into())?;
        let length = browser.len();

        // Store the browser, even if there is a
        match self.browser.write() {
            Ok(mut lock) => *lock = Some(browser),
            Err(mut lock_e) => {
                dbg!("Lock was poisened");
                **lock_e.get_mut() = Some(browser);
                self.browser.clear_poison();
            }
        }

        Ok(ResponseData::LoadedDirectory(length as u32))
    }

    fn handle_request(
        &mut self,
        cmd: Commands,
    ) -> ResponseAction<Result<ResponseData, ModelError>> {
        match cmd {
            Commands::LoadPhoto(index) => ResponseAction::Respond(self.load_photo(index)),
            Commands::LoadRawPhoto(index) => ResponseAction::Respond(self.load_raw_photo(index)),
            Commands::LoadThumbnail(index) => ResponseAction::Respond(self.load_preview(index)),

            Commands::LoadDirectory(path) => ResponseAction::Respond(self.load_directory(path)),

            Commands::AdjustImagesettings(id, settings) => {
                self.browser
                    .write()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .mut_at_index(id as usize)
                    .set_settings(settings);

                // Nothing to return
                // None

                ResponseAction::Respond(self.load_photo(id))
            }

            Commands::SetNormalFilter(index, filter_state) => {
                let mut browser = self.browser.write().unwrap();
                let container = browser.as_mut().unwrap().mut_at_index(index as usize);

                let mut filter = container.filter();
                filter.filter = filter_state;
                container.set_filter(filter);

                ResponseAction::Nothing
            }

            Commands::SetSchermFilter(index, scherm) => {
                let mut browser = self.browser.write().unwrap();
                let container = browser.as_mut().unwrap().mut_at_index(index as usize);

                let mut filter = container.filter();
                filter.scherm = scherm;
                container.set_filter(filter);

                ResponseAction::Nothing
            }

            Commands::SaveState => {
                let browser = self.browser.read().unwrap();
                browser.as_ref().unwrap().save_to_disk();

                // Do nothing after saving state, previously exited here, but didn't seem like a
                // good plan.
                return ResponseAction::Nothing;
            }

            Commands::KillThread => {
                // Exit to close the thread, could later add a specific command
                // to exit more controlled.
                ResponseAction::Exit
            }
        }
    }

    fn event_loop(mut self) {
        let receiver = self.receiver.clone();
        for msg in receiver {
            let cmd = msg.command();

            let return_value = self.handle_request(cmd);

            match return_value {
                ResponseAction::Respond(resp) => self
                    .response_sender
                    .send(Response {
                        request: msg,
                        value: resp,
                    })
                    .unwrap(),

                // We do nothing, so continue to next msg
                ResponseAction::Nothing => continue,

                // Close the thread
                ResponseAction::Exit => return,
            }
        }
    }

    pub fn run(self) -> JoinHandle<()> {
        thread::spawn(move || self.event_loop())
    }
}
