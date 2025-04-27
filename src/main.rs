use gpui::{App, AppContext, Application};

mod audio;
mod ui;

use audio::mic::{get_config_sample_rate, get_mic_stream};
use audio::whisper::WhisperTranscriber;
use cpal::traits::StreamTrait;
use std::thread;

fn main() {
    // Set up audio buffer and transcriber
    let sample_rate = get_config_sample_rate();
    let mut transcriber = WhisperTranscriber::new("./models/ggml-small.en.bin", sample_rate)
        .expect("Failed to load model");
    // Force initial calibration to be sensitive
    transcriber.set_calibration_thresholds(0.00005, 0.0001);
    let buffer = transcriber.get_buffer_handle();
    let rx = transcriber.start_background_processing();

    // Start mic stream
    let stream = get_mic_stream(buffer.clone()).expect("Failed to start mic stream");
    stream.play().expect("Failed to play stream");

    // Spawn a thread to listen for transcriptions
    thread::spawn(move || {
        while let Ok(transcription) = rx.recv() {
            println!("[JARVIS] Heard: {}", transcription);
            // Here you could trigger actions, update UI, etc.
        }
    });

    // Start the UI as before
    Application::new().run(|cx: &mut App| {
        cx.open_window(ui::window::create_window_options(cx), |_window, cx| {
            cx.new(|_cx| ui::components::MainContainer::default())
        })
        .unwrap();
    })
}
