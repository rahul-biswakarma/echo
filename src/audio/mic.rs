use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{BuildStreamError, Stream, StreamConfig};

pub fn get_mic_stream() -> Result<Stream, BuildStreamError> {
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

    device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            println!("data: {:?}", data);
        },
        move |err| {
            // react to errors here.
            println!("Error in stream: {:?}", err);
        },
        None, // None=blocking, Some(Duration)=timeout
    )
}
