use gpui::{App, AppContext, Application};
use std::path::PathBuf;

mod audio;
mod ui;

fn main() {
    // Use the local model file
    let model_path = ensure_whisper_model().expect("Failed to ensure Whisper model");

    println!("Using Whisper model at: {}", model_path.display());

    Application::new().run(|cx: &mut App| {
        cx.open_window(ui::window::create_window_options(cx), |_window, cx| {
            cx.new(|_cx| ui::components::MainContainer)
        })
        .unwrap();
    })
}

fn ensure_whisper_model() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Use the local model in the project directory
    let model_path = PathBuf::from("./models/ggml-small.en.bin");

    // Check if model exists
    if !model_path.exists() {
        println!(
            "Whisper model not found at {}. Please make sure the model file exists.",
            model_path.display()
        );

        // Return an error if the model doesn't exist
        return Err(
            "Whisper model not found. Please ensure the model is in the correct location.".into(),
        );
    }

    Ok(model_path)
}
