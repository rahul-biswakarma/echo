use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BuildStreamError, SampleRate, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

pub fn get_mic_stream(audio_buffer: Arc<Mutex<Vec<f32>>>) -> Result<Stream, BuildStreamError> {
    let host = cpal::default_host();

    // Try to get input device, return early with error if not available
    let device = match host.default_input_device() {
        Some(device) => device,
        None => {
            eprintln!("No input device available");
            return Err(BuildStreamError::DeviceNotAvailable);
        }
    };

    println!("Found input device: {:?}", device.name());

    // Try to get supported configs, return early with error if not available
    let supported_configs_range = match device.supported_input_configs() {
        Ok(configs) => configs,
        Err(err) => {
            eprintln!("Error querying configs: {:?}", err);
            return Err(BuildStreamError::DeviceNotAvailable);
        }
    };

    // Convert to a vector to avoid consuming the iterator
    let configs: Vec<_> = supported_configs_range.collect();

    if configs.is_empty() {
        eprintln!("No supported audio configurations found");
        return Err(BuildStreamError::DeviceNotAvailable);
    }

    // Print all available configs for debugging
    println!("Available configurations:");
    for (i, config) in configs.iter().enumerate() {
        println!(
            "  Config {}: {:?} - Sample rate range: {:?} - {:?}",
            i,
            config.sample_format(),
            config.min_sample_rate(),
            config.max_sample_rate()
        );
    }

    // Try multiple fallback strategies to find a working configuration
    let preferred_sample_rates = [16000, 44100, 48000];

    let mut selected_config = None;

    // First try: Look for configs that support our preferred sample rates
    for &rate in &preferred_sample_rates {
        let preferred_sample_rate = SampleRate(rate);

        for config in &configs {
            if config.min_sample_rate() <= preferred_sample_rate
                && config.max_sample_rate() >= preferred_sample_rate
            {
                selected_config = Some(config.with_sample_rate(preferred_sample_rate));
                println!("Selected config with preferred sample rate: {}Hz", rate);
                break;
            }
        }

        if selected_config.is_some() {
            break;
        }
    }

    // Second try: Just use the first config with its max sample rate
    if selected_config.is_none() && !configs.is_empty() {
        selected_config = Some(configs[0].with_max_sample_rate());
        println!("Falling back to first available config with max sample rate");
    }

    // If still no config, return error
    let config = match selected_config {
        Some(config) => config,
        None => {
            eprintln!("Could not find a usable audio configuration");
            return Err(BuildStreamError::DeviceNotAvailable);
        }
    };

    let sample_format = config.sample_format();
    let config: StreamConfig = config.into();

    println!(
        "Using audio input with sample rate: {}Hz",
        config.sample_rate.0
    );

    // Build the stream
    match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                process_audio_input(data, &audio_buffer);
            },
            move |err| {
                eprintln!("Error in stream: {:?}", err);
            },
            None,
        ),
        // Support for I16 format
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                // Convert i16 to f32
                let float_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                process_audio_input(&float_data, &audio_buffer);
            },
            move |err| {
                eprintln!("Error in stream: {:?}", err);
            },
            None,
        ),
        // Support for U16 format
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                // Convert u16 to f32
                let float_data: Vec<f32> = data
                    .iter()
                    .map(|&s| ((s as i32) - 32768) as f32 / 32768.0)
                    .collect();
                process_audio_input(&float_data, &audio_buffer);
            },
            move |err| {
                eprintln!("Error in stream: {:?}", err);
            },
            None,
        ),
        _ => {
            eprintln!("Unsupported sample format");
            Err(BuildStreamError::DeviceNotAvailable)
        }
    }
}

// Process input audio data with some basic preprocessing
fn process_audio_input(data: &[f32], audio_buffer: &Arc<Mutex<Vec<f32>>>) {
    // Keep a reasonable buffer size (increased to 10 seconds at 16kHz)
    // Whisper needs at least 1 second of audio
    const MAX_BUFFER_SIZE: usize = 16000 * 10;

    let mut buffer = audio_buffer.lock().unwrap();

    // Add data to the buffer
    buffer.extend_from_slice(data);

    // Trim the buffer to keep it at a reasonable size
    let current_len = buffer.len();
    if current_len > MAX_BUFFER_SIZE {
        buffer.drain(0..(current_len - MAX_BUFFER_SIZE));
    }
}

pub fn get_config_sample_rate() -> u32 {
    let host = cpal::default_host();

    // Try to get input device, return fallback if not available
    let device = match host.default_input_device() {
        Some(device) => device,
        None => {
            eprintln!("No input device available, using fallback sample rate");
            return 16000; // Fallback to 16kHz
        }
    };

    // Try to get supported configs, return fallback if not available
    let supported_configs = match device.supported_input_configs() {
        Ok(configs) => configs,
        Err(err) => {
            eprintln!(
                "Error querying configs: {:?}, using fallback sample rate",
                err
            );
            return 16000; // Fallback to 16kHz
        }
    };

    // Convert to a vector to avoid consuming the iterator
    let configs: Vec<_> = supported_configs.collect();

    if configs.is_empty() {
        eprintln!("No supported audio configurations found, using fallback sample rate");
        return 16000; // Fallback to 16kHz
    }

    // Try to find a config with 16kHz, which is good for speech
    let preferred_sample_rate = SampleRate(16000);

    // Try to find a config with our preferred sample rate
    for config in &configs {
        if config.min_sample_rate() <= preferred_sample_rate
            && config.max_sample_rate() >= preferred_sample_rate
        {
            return 16000;
        }
    }

    // If no config supports 16kHz, fall back to the first config's max rate
    if !configs.is_empty() {
        return configs[0].max_sample_rate().0;
    }

    // Final fallback
    16000
}
