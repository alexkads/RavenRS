//! Sound to spectrum and back, as the voices write it: windows of a periodic Hann,
//! overlapped by a hop.

use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;

/// A periodic Hann window of `size` samples.
pub(crate) fn hann(size: usize) -> Vec<f32> {
    (0..size)
        .map(|place| {
            let turn = std::f32::consts::TAU * place as f32 / size as f32;
            0.5 - 0.5 * turn.cos()
        })
        .collect()
}

/// The magnitude and angle of each bin of each window, the sound padded by reflection
/// at both ends as `torch.stft` does when centred.
pub(crate) fn stft(sound: &[f32], size: usize, hop: usize, window: &[f32]) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let half = size / 2;
    let last = sound.len() as isize - 1;
    let sample = |place: isize| -> f32 {
        let mut place = place;
        if place < 0 {
            place = -place;
        }
        if place > last {
            place = 2 * last - place;
        }
        sound[place.clamp(0, last) as usize]
    };
    let count = sound.len() / hop + 1;
    let fft = FftPlanner::new().plan_fft_forward(size);
    let bins = size / 2 + 1;
    let (mut magnitudes, mut angles) = (Vec::with_capacity(count), Vec::with_capacity(count));
    let mut buffer = vec![Complex32::new(0.0, 0.0); size];
    for frame in 0..count {
        let start = (frame * hop) as isize - half as isize;
        for (place, value) in buffer.iter_mut().enumerate() {
            *value = Complex32::new(sample(start + place as isize) * window[place], 0.0);
        }
        fft.process(&mut buffer);
        magnitudes.push(buffer[..bins].iter().map(|value| value.norm()).collect());
        angles.push(buffer[..bins].iter().map(|value| value.im.atan2(value.re)).collect());
    }
    (magnitudes, angles)
}

/// Sound from the magnitude and angle of each bin of each window, overlapped and added
/// with the window and divided by the windows' sum: `(frames - 1) * hop + size` samples,
/// which the caller trims as its spectrum was padded.
pub(crate) fn inverse_stft(
    magnitudes: &[Vec<f32>],
    angles: &[Vec<f32>],
    size: usize,
    hop: usize,
    window: &[f32],
) -> Vec<f32> {
    if magnitudes.is_empty() {
        return Vec::new();
    }
    let bins = size / 2 + 1;
    let fft = FftPlanner::new().plan_fft_inverse(size);
    let length = (magnitudes.len() - 1) * hop + size;
    let (mut sound, mut envelope) = (vec![0.0_f32; length], vec![0.0_f32; length]);
    let mut buffer = vec![Complex32::new(0.0, 0.0); size];
    for (index, (magnitude, angle)) in magnitudes.iter().zip(angles).enumerate() {
        for bin in 0..bins {
            let value = Complex32::from_polar(magnitude[bin], angle[bin]);
            buffer[bin] = value;
            if bin > 0 && bin < size - bin {
                buffer[size - bin] = value.conj();
            }
        }
        fft.process(&mut buffer);
        let start = index * hop;
        for place in 0..size {
            sound[start + place] += buffer[place].re / size as f32 * window[place];
            envelope[start + place] += window[place] * window[place];
        }
    }
    sound
        .iter()
        .zip(&envelope)
        .map(|(sound, envelope)| if *envelope > 1e-11 { sound / envelope } else { 0.0 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_comes_back_from_its_spectrum() {
        let sound: Vec<f32> = (0..400).map(|place| (place as f32 * 0.07).sin() * 0.5).collect();
        let window = hann(20);
        let (magnitudes, angles) = stft(&sound, 20, 5, &window);
        assert_eq!(magnitudes.len(), 400 / 5 + 1);
        let back = inverse_stft(&magnitudes, &angles, 20, 5, &window);
        let back = &back[10..10 + 400];
        let worst = sound.iter().zip(back).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
        assert!(worst < 1e-4, "{worst}");
    }
}
