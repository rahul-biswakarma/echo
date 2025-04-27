use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::vec::Vec;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Clone)]
pub struct WhisperTranscriber {
    context: Arc<WhisperContext>,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    debug_mode: bool,
    // Auto-calibration values
    auto_calibration: Arc<Mutex<AutoCalibration>>,
    // For background processing
    condvar: Arc<Condvar>,
    sender: Option<Sender<String>>, // For sending transcriptions
}

// Structure to hold auto-calibration data
#[derive(Clone)]
struct AutoCalibration {
    // Adjustable thresholds based on observed audio
    noise_floor: f32,
    voice_threshold: f32,
    // Tracking values
    max_observed_amplitude: f32,
    last_update: Instant,
    calibration_count: usize,
}

impl Default for AutoCalibration {
    fn default() -> Self {
        Self {
            noise_floor: 0.001,
            voice_threshold: 0.002,
            max_observed_amplitude: 0.0,
            last_update: Instant::now(),
            calibration_count: 0,
        }
    }
}

impl WhisperTranscriber {
    pub fn new(
        model_path: impl AsRef<Path>,
        sample_rate: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let model_path_str = model_path.as_ref().to_str().ok_or("Invalid path")?;
        let context =
            WhisperContext::new_with_params(model_path_str, WhisperContextParameters::default())?;

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let auto_calibration = Arc::new(Mutex::new(AutoCalibration::default()));

        Ok(Self {
            context: Arc::new(context),
            buffer,
            sample_rate,
            debug_mode: true, // Set to true to see debug output
            auto_calibration,
            condvar: Arc::new(Condvar::new()),
            sender: None,
        })
    }

    pub fn get_buffer_handle(&self) -> Arc<Mutex<Vec<f32>>> {
        self.buffer.clone()
    }

    // Detect if there's voice activity in the audio data
    fn detect_voice_activity(&self, audio_data: &[f32], threshold: f32) -> bool {
        if audio_data.is_empty() {
            return false;
        }

        // First method: Calculate RMS energy
        let sum_squares: f32 = audio_data.iter().map(|sample| sample * sample).sum();
        let rms = (sum_squares / audio_data.len() as f32).sqrt();

        // Second method: Count peaks exceeding threshold
        let peak_threshold = threshold * 0.5; // Lower threshold for peak counting
        let peaks = audio_data
            .iter()
            .filter(|&&s| s.abs() > peak_threshold)
            .count();
        let peak_ratio = peaks as f32 / audio_data.len() as f32;

        // Third method: Check if max amplitude exceeds higher threshold
        let max_amplitude = audio_data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        if self.debug_mode {
            println!(
                "Voice detection - RMS: {:.6}, Max amplitude: {:.6}, Peak ratio: {:.6}",
                rms, max_amplitude, peak_ratio
            );
        }

        // Return true if any of the methods detect voice
        rms > threshold || peak_ratio > 0.001 || max_amplitude > threshold * 3.0
    }

    // Simple noise reduction by applying a noise gate
    fn apply_noise_gate(&self, audio_data: &[f32], threshold: f32) -> Vec<f32> {
        // Use a very low threshold to keep most sounds
        let actual_threshold = threshold * 0.3;

        audio_data
            .iter()
            .map(|&sample| {
                if sample.abs() < actual_threshold {
                    0.0
                } else {
                    sample
                }
            })
            .collect()
    }

    // Normalize audio to increase voice volume
    fn normalize_audio(&self, audio_data: &[f32]) -> Vec<f32> {
        if audio_data.is_empty() {
            return Vec::new();
        }

        // Find the maximum amplitude
        let max_amplitude = audio_data
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0_f32, |a, b| a.max(b));

        // Always normalize to ensure consistent volume
        // Scale to desired level (0.8 avoids clipping)
        let target_amplitude = 0.8;
        // If audio is very quiet, use a higher scale factor with a minimum boost
        let scale_factor = if max_amplitude < 0.001 {
            // Apply a minimum boost for very quiet audio
            800.0
        } else if max_amplitude < 0.1 {
            // Apply a significant boost for quiet but detectable audio
            target_amplitude / max_amplitude
        } else {
            // Normal normalization for reasonably loud audio
            target_amplitude / max_amplitude.max(0.1)
        };

        if self.debug_mode {
            println!(
                "Normalizing audio - Max amplitude: {:.6}, Scale factor: {:.2}",
                max_amplitude, scale_factor
            );
        }

        audio_data
            .iter()
            .map(|&sample| {
                let normalized = sample * scale_factor;
                // Clip to avoid distortion
                normalized.max(-1.0).min(1.0)
            })
            .collect()
    }

    pub fn process_audio(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set parameters to improve transcription
        params.set_translate(false);
        params.set_language(Some("en")); // Set to English
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Additional params to improve performance
        params.set_n_threads(4); // Use more threads for processing
        params.set_token_timestamps(true); // Enable token timestamps
        params.set_no_context(true); // Process each chunk independently
        params.set_single_segment(true); // Process as single segment
        // params.set_beam_size(1); // Faster beam search

        // Clone the buffer to avoid holding the lock during processing
        let raw_audio_data = {
            let buffer = self.buffer.lock().unwrap();
            buffer.clone()
        };

        // Skip processing if buffer is empty
        if raw_audio_data.is_empty() {
            return Ok(String::new());
        }

        // Update auto-calibration based on latest audio
        self.update_calibration(&raw_audio_data);

        // Print some debug info about the audio
        if self.debug_mode {
            println!(
                "Processing audio buffer with {} samples",
                raw_audio_data.len()
            );

            // Calculate max amplitude to help with debugging
            let max_amplitude = raw_audio_data
                .iter()
                .map(|&s| s.abs())
                .fold(0.0f32, f32::max);
            println!("Max amplitude in buffer: {:.6}", max_amplitude);
        }

        // Get auto-calibrated thresholds
        let (noise_threshold, voice_threshold) = self.get_calibrated_thresholds();

        // Apply noise gate to reduce background noise
        let filtered_audio = self.apply_noise_gate(&raw_audio_data, noise_threshold);

        // Check if there's meaningful voice content
        let has_voice = self.detect_voice_activity(&filtered_audio, voice_threshold);
        if self.debug_mode {
            println!(
                "Voice detected: {} (thresholds: noise={:.6}, voice={:.6})",
                has_voice, noise_threshold, voice_threshold
            );
        }

        // Always process the audio if in debug mode, otherwise only if voice is detected
        if !has_voice && !self.debug_mode {
            return Ok(String::new());
        }

        // Normalize audio to improve voice detection
        let processed_audio = self.normalize_audio(&filtered_audio);

        // Resample the audio if needed - Whisper expects 16kHz
        let resampled_audio = if self.sample_rate != 16000 {
            if self.debug_mode {
                println!("Resampling from {}Hz to 16000Hz", self.sample_rate);
            }
            self.simple_resample(&processed_audio, self.sample_rate, 16000)
        } else {
            processed_audio
        };

        // Ensure audio is at least 1 second long by padding with silence
        // Whisper expects at least 1 second (16000 samples at 16kHz)
        let padded_audio = if resampled_audio.len() < 16000 {
            if self.debug_mode {
                println!(
                    "Padding audio from {} ms to 1000 ms",
                    resampled_audio.len() * 1000 / 16000
                );
            }
            let mut padded = resampled_audio.clone();
            // Add silence at the end to reach 1 second
            padded.resize(16000, 0.0);
            padded
        } else {
            resampled_audio
        };

        // Create a state for inference
        let mut state = self.context.create_state()?;

        // Run inference on the processed audio
        state.full(params, &padded_audio)?;

        // Get the results
        let num_segments = state.full_n_segments()?;
        let mut result = String::new();

        for i in 0..num_segments {
            let segment_text = state.full_get_segment_text(i)?;
            result.push_str(&segment_text);
            result.push(' ');
        }

        Ok(result.trim().to_string())
    }

    pub fn clear_buffer(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();
    }

    pub fn add_audio_data(&self, data: &[f32]) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.extend_from_slice(data);
        // Notify background thread
        self.condvar.notify_one();
    }

    // Simple linear resampling from source_rate to target_rate
    fn simple_resample(&self, audio: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
        if source_rate == target_rate {
            return audio.to_vec();
        }

        let src_len = audio.len();
        let target_len = (src_len as f32 * (target_rate as f32 / source_rate as f32)) as usize;
        let mut resampled = Vec::with_capacity(target_len);

        for i in 0..target_len {
            let src_idx = (i as f32 * (source_rate as f32 / target_rate as f32)) as usize;
            if src_idx < src_len {
                resampled.push(audio[src_idx]);
            } else {
                resampled.push(0.0);
            }
        }

        resampled
    }

    // Update calibration based on incoming audio
    fn update_calibration(&self, audio_data: &[f32]) {
        let mut calibration = self.auto_calibration.lock().unwrap();

        // Get the maximum amplitude from this audio chunk
        let max_amplitude = audio_data.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        // Update our max observed amplitude if this is higher
        if max_amplitude > calibration.max_observed_amplitude {
            calibration.max_observed_amplitude = max_amplitude;
        }

        // Recalibrate every 10 seconds
        if calibration.last_update.elapsed() > Duration::from_secs(10) {
            // Adjust thresholds based on observed audio
            if calibration.calibration_count > 0 {
                // Set noise floor to 10% of max observed amplitude
                calibration.noise_floor = calibration.max_observed_amplitude * 0.1;
                // Set voice threshold to 20% of max observed amplitude
                calibration.voice_threshold = calibration.max_observed_amplitude * 0.2;

                if self.debug_mode {
                    println!(
                        "Auto-calibration updated - Max amplitude: {:.6}, Noise threshold: {:.6}, Voice threshold: {:.6}",
                        calibration.max_observed_amplitude,
                        calibration.noise_floor,
                        calibration.voice_threshold
                    );
                }
            }

            // Reset for next period
            calibration.last_update = Instant::now();
            calibration.calibration_count += 1;
        }
    }

    // Get current calibrated thresholds
    fn get_calibrated_thresholds(&self) -> (f32, f32) {
        let calibration = self.auto_calibration.lock().unwrap();
        // Use much lower minimums for sensitivity
        let noise_threshold = calibration.noise_floor.max(0.00005);
        let voice_threshold = calibration.voice_threshold.max(0.0001);
        (noise_threshold, voice_threshold)
    }

    /// Start the always-on background processing thread.
    /// Returns a Receiver<String> for transcriptions.
    pub fn start_background_processing(&mut self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        self.sender = Some(tx.clone());
        let buffer = self.buffer.clone();
        let context = self.context.clone();
        let sample_rate = self.sample_rate;
        let debug_mode = self.debug_mode;
        let auto_calibration = self.auto_calibration.clone();
        let condvar = self.condvar.clone();

        thread::spawn(move || {
            enum State {
                Idle,
                Listening,
                Processing,
            }
            let mut state = State::Idle;
            let mut last_processed = 0;
            let mut listen_buffer = Vec::new();
            let mut silence_counter = 0;
            // For sliding window
            let window_size = 16000; // 1s
            let hop_size = 4000; // 0.25s
            let mut wake_window_start = 0;
            loop {
                let mut buf = buffer.lock().unwrap();
                // Sliding window: only proceed if enough for a full window
                if buf.len() - wake_window_start < window_size {
                    buf = condvar.wait(buf).unwrap();
                    continue;
                }
                let chunk = buf[wake_window_start..wake_window_start + window_size].to_vec();
                drop(buf);

                match state {
                    State::Idle => {
                        // Wake word detection: run Whisper on chunk, look for 'echo'
                        let mut state_obj = context.create_state().unwrap();
                        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                        params.set_translate(false);
                        params.set_language(Some("en"));
                        params.set_print_progress(false);
                        params.set_print_special(false);
                        params.set_print_realtime(false);
                        params.set_print_timestamps(false);
                        params.set_n_threads(2);
                        params.set_token_timestamps(false);
                        params.set_no_context(true);
                        params.set_single_segment(true);
                        let mut padded = chunk.clone();
                        if padded.len() < window_size {
                            padded.resize(window_size, 0.0);
                        }
                        let _ = state_obj.full(params, &padded);
                        let mut found = false;
                        let num_segments = state_obj.full_n_segments().unwrap_or(0);
                        for i in 0..num_segments {
                            if let Ok(text) = state_obj.full_get_segment_text(i) {
                                if debug_mode {
                                    println!("[Wake] Detected: {}", text);
                                }
                                if text.to_lowercase().contains("echo") {
                                    found = true;
                                    break;
                                }
                            }
                        }
                        if found {
                            let _ = tx.send("__listening__".to_string());
                            state = State::Listening;
                            listen_buffer.clear();
                            // Start listening from the start of the window (to not miss anything)
                            last_processed = wake_window_start;
                            silence_counter = 0;
                        }
                        // Advance window by hop_size (0.25s)
                        wake_window_start += hop_size;
                        // Prevent runaway growth
                        if wake_window_start > 32000 {
                            // Drop old samples if buffer grows too large
                            let mut buf = buffer.lock().unwrap();
                            if buf.len() > wake_window_start {
                                buf.drain(0..wake_window_start);
                            }
                            wake_window_start = 0;
                        }
                    }
                    State::Listening => {
                        // Buffer audio, check for silence
                        let mut buf = buffer.lock().unwrap();
                        // Always work in 1s windows for listening
                        if buf.len() - last_processed < window_size {
                            drop(buf);
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            continue;
                        }
                        let chunk = buf[last_processed..last_processed + window_size].to_vec();
                        drop(buf);
                        listen_buffer.extend_from_slice(&chunk);
                        // Voice activity detection
                        let sum_squares: f32 = chunk.iter().map(|s| s * s).sum();
                        let rms = (sum_squares / chunk.len() as f32).sqrt();
                        let max_amplitude = chunk.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
                        let has_voice = rms > 0.0001 || max_amplitude > 0.0003;
                        if has_voice {
                            silence_counter = 0;
                        } else {
                            silence_counter += 1;
                        }
                        // If 1s of silence, stop listening and process
                        if silence_counter >= 1 {
                            state = State::Processing;
                        }
                        last_processed += window_size;
                    }
                    State::Processing => {
                        // Process listen_buffer with Whisper
                        let mut state_obj = context.create_state().unwrap();
                        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                        params.set_translate(false);
                        params.set_language(Some("en"));
                        params.set_print_progress(false);
                        params.set_print_special(false);
                        params.set_print_realtime(false);
                        params.set_print_timestamps(false);
                        params.set_n_threads(4);
                        params.set_token_timestamps(true);
                        params.set_no_context(true);
                        params.set_single_segment(true);
                        let mut padded = listen_buffer.clone();
                        if padded.len() < window_size {
                            padded.resize(window_size, 0.0);
                        }
                        let _ = state_obj.full(params, &padded);
                        let num_segments = state_obj.full_n_segments().unwrap_or(0);
                        let mut result = String::new();
                        for i in 0..num_segments {
                            if let Ok(text) = state_obj.full_get_segment_text(i) {
                                result.push_str(&text);
                                result.push(' ');
                            }
                        }
                        let result = result.trim().to_string();
                        if !result.is_empty() {
                            let _ = tx.send(result);
                        }
                        // Reset for next cycle
                        state = State::Idle;
                        listen_buffer.clear();
                        silence_counter = 0;
                        // Reset window for wake word detection
                        wake_window_start = last_processed;
                    }
                }
            }
        });
        rx
    }

    /// Notify the background thread that new audio is available
    pub fn notify_audio(&self) {
        self.condvar.notify_one();
    }

    /// Set initial calibration thresholds for sensitivity
    pub fn set_calibration_thresholds(&self, noise_floor: f32, voice_threshold: f32) {
        let mut cal = self.auto_calibration.lock().unwrap();
        cal.noise_floor = noise_floor;
        cal.voice_threshold = voice_threshold;
    }
}
