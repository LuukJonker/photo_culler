// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod image_loader;

use std::error::Error;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let img = image::open("test.png").expect("DJFKD");
    ui.set_current_image(image_loader::dynamic_image_to_slint_image(img));

    ui.run()?;

    Ok(())
}
