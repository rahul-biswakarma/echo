# Whisper Speech Transcription App

This application uses the Whisper speech recognition model to transcribe speech from your microphone in real-time.

## Requirements

- Rust (latest stable version)
- A microphone
- A Whisper model file

## Installation

1. Clone this repository
2. Download a Whisper model from the official repository:
   - Visit https://huggingface.co/ggerganov/whisper.cpp/tree/main
   - Download a model file (recommended: `ggml-small.en.bin` for better balance of speed and accuracy)
   - Place the model file in the `models` directory

## Running the Application

```
cargo run
```

The application will start listening to your microphone and transcribe your speech every 5 seconds. You can also click the "Transcribe Now" button to manually trigger transcription.

## Models

Whisper comes with various model sizes:
- `tiny`: Smallest model, faster but less accurate
- `base`: Balanced between speed and accuracy
- `small`, `medium`, `large`: Increasingly accurate but slower and more resource-intensive

## Troubleshooting

If you encounter issues with the microphone, make sure your system's input device is correctly configured.

If the transcription is not working, ensure the model file is correctly placed at `./models/ggml-small.en.bin` or update the path in the code to match your model file's location.

## License

MIT
