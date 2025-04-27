use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BuildStreamError, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

pub fn get_mic_stream(audio_buffer: Arc<Mutex<Vec<f32>>>) -> Result<Stream, BuildStreamError> {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .expect("no input device available");

    // println!("device: {:?}", device);

    let mut supported_configs_range = device
        .supported_input_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!")
        .with_max_sample_rate();

    let config: StreamConfig = supported_config.into();
    let sample_rate = config.sample_rate.0;

    device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            // Add data to the buffer
            let mut buffer = audio_buffer.lock().unwrap();
            buffer.extend_from_slice(data);

            // Keep a reasonable buffer size (approx. 30 seconds at 16kHz)
            const MAX_BUFFER_SIZE: usize = 16000 * 30;
            let current_len = buffer.len();
            if current_len > MAX_BUFFER_SIZE {
                buffer.drain(0..(current_len - MAX_BUFFER_SIZE));
            }
        },
        move |err| {
            // react to errors here.
            println!("Error in stream: {:?}", err);
        },
        None, // None=blocking, Some(Duration)=timeout
    )
}

pub fn get_config_sample_rate() -> u32 {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("no input device available");

    let mut supported_configs_range = device
        .supported_input_configs()
        .expect("error while querying configs");

    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!")
        .with_max_sample_rate();

    supported_config.sample_rate().0
}
