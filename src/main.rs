use hound;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use fluidlite::{Settings, Synth};

const SAMPLE_RATE: u32 = 44100;
const FFT_SIZE: usize = 1024;
const AGGREGATE_SIZE: usize = 44100; // Aggregate over 1 second (adjust as needed)

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input_file = "input.wav";
    let output_file = "output.wav";
    let soundfont_file = "soundfonts/mother3.sf2"; // Change this to the path of your SoundFont file

    let mut reader = hound::WavReader::open(input_file)?;
    let spec = reader.spec();
    let samples: Vec<f32> = reader.samples::<i16>()
                                   .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                                   .collect();

    let mut pitch_timing_info = Vec::new();
    let mut current_chunk = Vec::new();
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut buffer = vec![Complex::zero(); FFT_SIZE];

    for (i, &sample) in samples.iter().enumerate() {
        current_chunk.push(sample);
        if current_chunk.len() >= AGGREGATE_SIZE || i == samples.len() - 1 {
            // Perform FFT on smaller chunks within the larger aggregate chunk
            let mut frequencies = Vec::new();
            for chunk in current_chunk.chunks(FFT_SIZE) {
                for (j, &s) in chunk.iter().enumerate() {
                    buffer[j] = Complex::new(s, 0.0);
                }
                fft.process(&mut buffer);

                let magnitudes: Vec<f32> = buffer.iter().map(|c| c.norm()).collect();
                let (max_index, _) = magnitudes.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap();
                let frequency = max_index as f32 * SAMPLE_RATE as f32 / FFT_SIZE as f32;

                if frequency >= 20.0 && frequency <= 20000.0 {
                    frequencies.push(frequency);
                }
            }

            if !frequencies.is_empty() {
                let avg_frequency: f32 = frequencies.iter().sum::<f32>() / frequencies.len() as f32;
                pitch_timing_info.push((avg_frequency, current_chunk.len() as f32 / SAMPLE_RATE as f32));
            }

            current_chunk.clear();
        }
    }

    // Initialize the synthesizer
    let settings = Settings::new().expect("Synth settings failed to initialize.");
    let synth = Synth::new(settings).expect("Synth failed to initialize.");
    synth.sfload(soundfont_file, true).expect("Soundfont failed to load.");

    // Generate synthesized audio
    let mut writer = hound::WavWriter::create(output_file, spec)?;

    for (frequency, duration) in pitch_timing_info {
        let num_samples = (duration * SAMPLE_RATE as f32) as usize;
        let midi_note = frequency_to_midi(frequency);

        // Ensure MIDI note is within valid range
        if midi_note > 127 {
            println!("Skipping out-of-range frequency: {}", frequency);
            continue;
        }

        println!("Freq: {}\nNote: {}", frequency, midi_note);
        synth.note_on(0, midi_note, 100).expect("Note could not be played!");

        let mut samples = vec![0.0; num_samples];
        synth.write(&mut samples[..]).expect("Synth could not write to buffer!");

        for sample in samples {
            writer.write_sample((sample * i16::MAX as f32) as i16)?;
        }
        
        synth.note_off(0, midi_note).expect("Note could not be stopped!");
    }

    Ok(())
}

fn frequency_to_midi(frequency: f32) -> u32 {
    let midi_note = 69.0 + 12.0 * (frequency / 440.0).log2();
    midi_note.round() as u32
}
