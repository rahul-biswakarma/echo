use crate::audio::whisper::WhisperTranscriber;
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb, rgba};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct SharedState {
    transcription: String,
    is_processing: bool,
    should_exit: bool,
}

pub struct MainContainer {
    transcriber: Option<WhisperTranscriber>,
    buffer: Option<Arc<Mutex<Vec<f32>>>>,
    stream: Option<cpal::Stream>,
    shared_state: Arc<Mutex<SharedState>>,
    update_thread: Option<thread::JoinHandle<()>>,
    ui_update_thread: Option<thread::JoinHandle<()>>,
}

impl MainContainer {
    fn init_whisper(&mut self, _cx: &mut Context<Self>) {
        // Initialize whisper if not already initialized
        if self.transcriber.is_none() {
            let model_path = PathBuf::from("./models/ggml-small.en.bin");
            let sample_rate = crate::audio::mic::get_config_sample_rate();

            match WhisperTranscriber::new(model_path, sample_rate) {
                Ok(transcriber) => {
                    let buffer_handle = transcriber.get_buffer_handle();

                    match crate::audio::mic::get_mic_stream(buffer_handle.clone()) {
                        Ok(stream) => {
                            self.transcriber = Some(transcriber.clone());
                            self.buffer = Some(buffer_handle);
                            self.stream = Some(stream);

                            // Start a background thread for audio processing
                            let transcriber_clone = transcriber;
                            let shared_state = self.shared_state.clone();

                            let update_thread = thread::spawn(move || {
                                loop {
                                    // Sleep first to let initial UI render
                                    thread::sleep(Duration::from_millis(1500));

                                    // Check if we should exit
                                    {
                                        let state = shared_state.lock().unwrap();
                                        if state.should_exit {
                                            break;
                                        }
                                    }

                                    // Process audio in background thread
                                    {
                                        let mut state = shared_state.lock().unwrap();
                                        if state.is_processing {
                                            continue;
                                        }
                                        state.is_processing = true;
                                        state.transcription = "Processing...".to_string();
                                    }

                                    // Process audio with Whisper
                                    match transcriber_clone.process_audio() {
                                        Ok(text) => {
                                            if !text.is_empty() {
                                                // Update shared state with new transcription
                                                let mut state = shared_state.lock().unwrap();
                                                state.transcription = text;
                                            }

                                            // Clear buffer after processing
                                            transcriber_clone.clear_buffer();
                                        }
                                        Err(err) => {
                                            eprintln!("Error during transcription: {:?}", err);
                                        }
                                    }

                                    // Mark processing as complete
                                    {
                                        let mut state = shared_state.lock().unwrap();
                                        state.is_processing = false;
                                    }
                                }
                            });

                            self.update_thread = Some(update_thread);

                            // Start a separate thread for UI updates
                            let shared_state_for_ui = self.shared_state.clone();
                            let ui_update_thread = thread::spawn(move || {
                                loop {
                                    // Sleep briefly to avoid excessive updates
                                    thread::sleep(Duration::from_millis(100));

                                    // Check if we should exit
                                    {
                                        let state = shared_state_for_ui.lock().unwrap();
                                        if state.should_exit {
                                            break;
                                        }
                                    }

                                    // Try to update the UI
                                    // This just periodically triggers a UI refresh
                                    // The actual UI will read the current state when it renders
                                }
                            });

                            self.ui_update_thread = Some(ui_update_thread);
                        }
                        Err(err) => {
                            eprintln!("Failed to create microphone stream: {:?}", err);
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Failed to initialize Whisper: {:?}", err);
                }
            }
        }
    }

    fn get_transcription(&self) -> String {
        let state = self.shared_state.lock().unwrap();
        state.transcription.clone()
    }
}

impl Drop for MainContainer {
    fn drop(&mut self) {
        // Signal threads to exit
        {
            let mut state = self.shared_state.lock().unwrap();
            state.should_exit = true;
        }

        // Wait for threads to finish
        if let Some(thread) = self.update_thread.take() {
            let _ = thread.join();
        }

        if let Some(thread) = self.ui_update_thread.take() {
            let _ = thread.join();
        }
    }
}

impl Default for MainContainer {
    fn default() -> Self {
        Self {
            transcriber: None,
            buffer: None,
            stream: None,
            shared_state: Arc::new(Mutex::new(SharedState {
                transcription: String::new(),
                is_processing: false,
                should_exit: false,
            })),
            update_thread: None,
            ui_update_thread: None,
        }
    }
}

impl Render for MainContainer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Initialize whisper on first render
        self.init_whisper(cx);

        // Get the current transcription
        let transcription = self.get_transcription();

        let display_text = if transcription.is_empty() {
            "Listening...".to_string()
        } else {
            transcription
        };

        div()
            .size(px(400.0))
            .flex()
            .justify_center()
            .items_center()
            .border_1()
            .border_color(rgba(0xFFFAFA80))
            .shadow_lg()
            .p(px(10.0))
            .children([div()
                .text_xl()
                .text_color(rgb(0xffffff))
                .children([display_text])])
    }
}
