use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use realfft::RealFftPlanner;
use std::sync::{Arc, Mutex};

pub struct VoiceData {
    pub frequency_hz: f32,
    pub amplitude: f32,
}

pub fn start_voice_capture(tx: crossbeam_channel::Sender<VoiceData>) {
    std::thread::Builder::new()
        .name("cymatrox-audio".into())
        .spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    eprintln!("[audio] no input device found");
                    return;
                }
            };

            let supported = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[audio] failed to get input config: {e}");
                    return;
                }
            };

            let sample_rate = supported.sample_rate() as usize;
            let channels = supported.channels() as usize;
            let buffer_size = 1024;
            let config: cpal::StreamConfig = supported.into();

            let data: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

            let data_clone = data.clone();
            let stream = match device.build_input_stream(
                config,
                move |samples: &[f32], _: &cpal::InputCallbackInfo| {
                    let mono: Vec<f32> = samples
                        .chunks(channels)
                        .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                        .collect();
                    if let Ok(mut buf) = data_clone.lock() {
                        buf.extend_from_slice(&mono);
                        if buf.len() > buffer_size * 2 {
                            let drain = buf.len() - buffer_size;
                            buf.drain(..drain);
                        }
                    }
                },
                |e| {
                    eprintln!("[audio] stream error: {e}");
                },
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[audio] failed to build stream: {e}");
                    return;
                }
            };

            if let Err(e) = stream.play() {
                eprintln!("[audio] failed to play stream: {e}");
                return;
            }

            eprintln!("[audio] voice capture started");

            let mut planner = RealFftPlanner::<f32>::new();
            let fft = planner.plan_fft_forward(buffer_size);

            loop {
                std::thread::sleep(std::time::Duration::from_millis(20));

                let samples = {
                    let Ok(mut buf) = data.lock() else {
                        continue;
                    };
                    if buf.len() < buffer_size {
                        continue;
                    }
                    let s: Vec<f32> = buf.drain(..buffer_size).collect();
                    s
                };

                let windowed: Vec<f32> = samples
                    .iter()
                    .enumerate()
                    .map(|(i, &s)| {
                        let w = 0.5
                            * (1.0 - (std::f32::consts::TAU * i as f32 / buffer_size as f32).cos());
                        s * w
                    })
                    .collect();

                let mut spectrum = vec![0.0f32; buffer_size];
                let mut output = vec![num_complex::Complex::default(); buffer_size / 2 + 1];
                spectrum[..buffer_size].copy_from_slice(&windowed);

                if fft.process(&mut spectrum, &mut output).is_err() {
                    continue;
                }

                let mut max_mag = 0.0f32;
                let mut max_bin = 1usize;
                for (i, c) in output.iter().enumerate().skip(1) {
                    let mag = c.norm();
                    if mag > max_mag {
                        max_mag = mag;
                        max_bin = i;
                    }
                }

                let freq = max_bin as f32 * sample_rate as f32 / buffer_size as f32;
                let rms: f32 =
                    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();

                let _ = tx.send(VoiceData {
                    frequency_hz: freq,
                    amplitude: rms,
                });
            }
        })
        .expect("failed to spawn audio thread");
}
