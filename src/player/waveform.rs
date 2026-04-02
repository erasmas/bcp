use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Extract a waveform (amplitude envelope) from MP3 data.
/// Returns a Vec of RMS amplitude values (0-100) at ~100 points per second
/// for smooth scrolling display.
pub fn extract_waveform(mp3_data: &[u8]) -> Vec<u64> {
    let cursor = std::io::Cursor::new(mp3_data.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let probed = match symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let mut format = probed.format;
    let track = match format.default_track() {
        Some(t) => t,
        None => return Vec::new(),
    };

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100) as f64;
    let track_id = track.id;
    let mut decoder = match symphonia::default::get_codecs().make(
        &track.codec_params,
        &DecoderOptions::default(),
    ) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    // Collect all samples into a flat buffer, then chunk into windows
    // Target: one amplitude value per 10ms (100 values per second)
    let samples_per_point = (sample_rate * 0.01) as usize; // 10ms windows
    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let spec = *decoded.spec();
        let num_samples = decoded.capacity();
        if num_samples == 0 {
            continue;
        }

        let mut sample_buf = SampleBuffer::<f32>::new(num_samples as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        // Mix to mono by averaging channels
        let channels = spec.channels.count().max(1);
        let samples = sample_buf.samples();
        for chunk in samples.chunks(channels) {
            let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
            all_samples.push(mono);
        }
    }

    if all_samples.is_empty() {
        return Vec::new();
    }

    // Compute RMS amplitude for each 10ms window
    let mut waveform = Vec::new();
    for chunk in all_samples.chunks(samples_per_point.max(1)) {
        let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
        // Scale to 0-100, applying some gain for visibility
        let val = (rms * 200.0).min(100.0) as u64;
        waveform.push(val);
    }

    waveform
}
