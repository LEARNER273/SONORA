use std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct FastFourierTransform {
    size: usize,
    window: Vec<f32>,
    bit_reversed: Vec<usize>,
}

impl FastFourierTransform {
    pub fn new(size: usize) -> Self {
        assert!(size.is_power_of_two(), "FFT size must be a power of two");

        // Precompute Hann window
        let mut window = Vec::with_capacity(size);
        for i in 0..size {
            let w = 0.5 * (1.0 - (2.0 * PI * i as f32 / (size as f32 - 1.0)).cos());
            window.push(w);
        }

        // Precompute bit-reversal indices
        let bits = size.trailing_zeros();
        let mut bit_reversed = Vec::with_capacity(size);
        for i in 0..size {
            bit_reversed.push(i.reverse_bits() >> (usize::BITS - bits));
        }

        Self {
            size,
            window,
            bit_reversed,
        }
    }

    /// Performs FFT on real audio input and returns frequency magnitude bins (half spectrum)
    pub fn compute_magnitudes(&self, input: &[f32], output_magnitudes: &mut [f32]) {
        let n = self.size;
        let half = n / 2;
        let mut real = vec![0.0f32; n];
        let mut imag = vec![0.0f32; n];

        // Apply window and bit reversal permutation
        for i in 0..n {
            let src_idx = if i < input.len() { i } else { 0 };
            let rev = self.bit_reversed[i];
            real[rev] = input[src_idx] * self.window[i];
            imag[rev] = 0.0;
        }

        // In-place Cooley-Tukey Radix-2 FFT
        let mut step = 2;
        while step <= n {
            let half_step = step / 2;
            let theta = -2.0 * PI / (step as f32);
            let w_step_real = theta.cos();
            let w_step_imag = theta.sin();

            let mut k = 0;
            while k < n {
                let mut w_real = 1.0f32;
                let mut w_imag = 0.0f32;

                for j in 0..half_step {
                    let idx_a = k + j;
                    let idx_b = idx_a + half_step;

                    let u_real = real[idx_a];
                    let u_imag = imag[idx_a];

                    let v_real = real[idx_b] * w_real - imag[idx_b] * w_imag;
                    let v_imag = real[idx_b] * w_imag + imag[idx_b] * w_real;

                    real[idx_a] = u_real + v_real;
                    imag[idx_a] = u_imag + v_imag;

                    real[idx_b] = u_real - v_real;
                    imag[idx_b] = u_imag - v_imag;

                    let next_w_real = w_real * w_step_real - w_imag * w_step_imag;
                    let next_w_imag = w_real * w_step_imag + w_imag * w_step_real;
                    w_real = next_w_real;
                    w_imag = next_w_imag;
                }
                k += step;
            }
            step *= 2;
        }

        // Calculate normalized magnitude spectrum (in dB normalized to [0.0, 1.0])
        let out_len = output_magnitudes.len().min(half);
        for i in 0..out_len {
            let mag = (real[i] * real[i] + imag[i] * imag[i]).sqrt() / (n as f32);
            let db = if mag > 1e-5 {
                20.0 * mag.log10()
            } else {
                -100.0
            };
            // Map -80 dBFS .. 0 dBFS to 0.0 .. 1.0
            output_magnitudes[i] = ((db + 80.0) / 80.0).clamp(0.0, 1.0);
        }
    }
}
