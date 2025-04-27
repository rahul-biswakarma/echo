use std::path::Path;
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Clone)]
pub struct WhisperTranscriber {
    context: Arc<WhisperContext>,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    debug_mode: bool,
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

        Ok(Self {
            context: Arc::new(context),
            buffer,
            sample_rate,
            debug_mode: true, // Set to true to see debug output
        })
    }

    pub fn get_buffer_handle(&self) -> Arc<Mutex<Vec<f32>>> {
        self.buffer.clone()
    }

    // Detect if there's voice activity in the audio data
    fn detect_voice_activity(&self, audio_data: &[f32], threshold: f32) -> bool {
        let energy_threshold = threshold; // Adjust as needed

        // Calculate RMS energy
        let sum_squares: f32 = audio_data.iter().map(|sample| sample * sample).sum();
        let rms = (sum_squares / audio_data.len() as f32).sqrt();

        rms > energy_threshold
    }

    // Simple noise reduction by applying a noise gate
    fn apply_noise_gate(&self, audio_data: &[f32], threshold: f32) -> Vec<f32> {
        audio_data
            .iter()
            .map(|&sample| {
                if sample.abs() < threshold {
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

        if max_amplitude > 0.01 {
            // Scale to desired level (0.8 avoids clipping)
            let target_amplitude = 0.8;
            let scale_factor = target_amplitude / max_amplitude;

            audio_data
                .iter()
                .map(|&sample| sample * scale_factor)
                .collect()
        } else {
            // If audio is very quiet, don't normalize
            audio_data.to_vec()
        }
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

        // Apply audio preprocessing with lower thresholds
        let noise_threshold = 0.005; // Lowered from 0.1
        let voice_threshold = 0.01; // Lowered from 0.1

        // Apply noise gate to reduce background noise
        let filtered_audio = self.apply_noise_gate(&raw_audio_data, noise_threshold);

        // Check if there's meaningful voice content
        let has_voice = self.detect_voice_activity(&filtered_audio, voice_threshold);
        if self.debug_mode {
            println!("Voice detected: {}", has_voice);
        }

        if !has_voice {
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
}
