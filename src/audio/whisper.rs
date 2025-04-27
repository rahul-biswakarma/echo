use std::path::Path;
use std::sync::{Arc, Mutex};
use std::vec::Vec;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperTranscriber {
    context: WhisperContext,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
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
            context,
            buffer,
            sample_rate,
        })
    }

    pub fn get_buffer_handle(&self) -> Arc<Mutex<Vec<f32>>> {
        self.buffer.clone()
    }

    pub fn process_audio(&self) -> Result<String, Box<dyn std::error::Error>> {
        let params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Clone the buffer to avoid holding the lock during processing
        let audio_data = {
            let buffer = self.buffer.lock().unwrap();
            buffer.clone()
        };

        // Skip processing if buffer is empty
        if audio_data.is_empty() {
            return Ok(String::new());
        }

        // Create a state for inference
        let mut state = self.context.create_state()?;

        // Run inference
        state.full(params, &audio_data)?;

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
}
