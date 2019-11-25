use num::Complex;

use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;

arg_enum! {
    #[derive(Clone)]
    #[allow(dead_code)]
    pub enum WindowFunc {
        Nuttall,
        Rectangle,
        Triangle,
        Gaussian,
    }
}

pub fn prep_signals(signals: &[Vec<f32>], window_type: WindowFunc) -> (Vec<f32>, usize, usize) {
    let num_stars = signals.len();
    let signal_max_len = signals
        .iter()
        .map(|signal| signal.len())
        .max()
        .expect("Problem getting the max signal length.");

    let window_func = match window_type {
        WindowFunc::Nuttall => nuttall_window,
        WindowFunc::Triangle => triangle_window,
        WindowFunc::Gaussian => gaussian_window,
        WindowFunc::Rectangle => |arr| { arr },
    };

    // Zero pad the results to make sure all signals have
    // same length regardless of window size. This does not
    // have any effect on output (except binning which is
    // discussed in the ni article below).
    //
    // Corresponds to perfect interpolation as pointed out by
    // https://math.stackexchange.com/questions/26432/
    //   discrete-fourier-transform-effects-of-zero-padding-compared-to-time-domain-inte
    // and with stated theorem referenced here
    // https://ccrma.stanford.edu/~jos/dft/Zero_Padding_Theorem_Spectral.html
    //
    // See here for an analysis of zero padding
    // - http://www.ni.com/tutorial/4880/en/
    let signals = signals
        .iter()
        .cloned()
        .into_iter()
        .flat_map(|mut signal| {
            let len = signal.len();
            signal = stddev_outlier_removal(signal);
            //signal = nuttall_window(signal);
            //signal = triangle_window(signal);

            signal = window_func(signal);

            signal.into_iter().chain(
                std::iter::repeat(0.0f32)
                    .take(signal_max_len - len),
            )
        })
        .collect::<Vec<f32>>();

    (signals, num_stars, signal_max_len)
}

pub fn prep_stars(signals: &[f32], num_stars:usize, signal_max_len: usize) -> AF_Array<f32> {
    let stars = AF_Array::new(
        &signals[..],
        // [ ] TODO 2nd term should be # of stars???
        AF_Dim4::new(&[signal_max_len as u64, num_stars as u64, 1, 1]),
    );

    stars
}

pub fn af_to_vec1d<T>(arr: &AF_Array<T>) -> Vec<T> where
    T: AF::HasAfEnum + std::clone::Clone + std::default::Default {
    let mut temp: Vec<T> = Vec::new();
    temp.resize(arr.elements(), Default::default());
    arr.lock();
    arr.host(&mut temp);
    arr.unlock();

    temp
}

#[allow(dead_code)]
pub fn af_to_vec2d<T>(arr: &AF_Array<T>, dim_1_len: usize) -> Vec<Vec<T>> where
    T: AF::HasAfEnum + std::clone::Clone + std::default::Default {
    let mut temp: Vec<T> = Vec::new();
    temp.resize(arr.elements(), Default::default());
    arr.lock();
    arr.host(&mut temp);
    arr.unlock();

    temp.chunks(dim_1_len).map(|chunk| Vec::from(chunk)).collect()
}

pub fn stars_dc_removal(stars: &AF_Array<f32>, signal_max_len: usize) -> AF_Array<f32> {
    // NOTE Remove DC constant of template to focus on signal
    //      - This is very important and will lead to false
    //        detection or searching for the wrong signal
    let stars_means = AF::mean(
        stars,
        0,
    );
    let stars_means = AF::tile(
        &stars_means,
        AF_Dim4::new(
            &[signal_max_len as u64, 1, 1, 1]
        ),
    );

    AF::sub(stars, &stars_means, false)
}

pub fn stars_fft(stars: &AF_Array<f32>, fft_len: usize, fft_half_len: usize) -> AF_Array<Complex<f32>> {
    let stars = {
        let fft_bs = AF::fft(stars, 1.0, fft_len as i64);
        AF::rows(&fft_bs, 0, fft_half_len as u64)
    };

    stars
}

fn stddev_outlier_removal(mut signal: Vec<f32>) -> Vec<f32> {
    // Implements a basic outlier removal scheme that replaces
    // any point that is beyond 3*stddev of the mean with a
    // neighboring point
    //
    // NOTE for now assumes outliers are positive and not negative (additive)

    let len = signal.len();

    let mean: f32 = signal.iter().sum();
    let mean = mean / len as f32;

    // Formula for the corrected sample standard deviation
    // from the Wikipedia article on standard deviation
    let stddev: f32 = signal.iter().map(|x| (x-mean).powf(2.0)).sum();
    let stddev = stddev / (len - 1) as f32;
    let stddev = stddev.sqrt();

    let threshold = mean + 3.0*stddev;

    // NOTE assumes that the signal is at least 2 length wide
    // and that their is not more that two errors in a row
    // NOTE last case extracted here to remove redundant if
    // statement in the for loop below
    if signal[len - 1] > threshold {
        signal[len - 1] = signal[len - 2];
    }

    for i in 0..len-1 {
        // NOTE for now does not handle more than two errors in a row
        // - it will back propagate these errors
        if signal[i] > threshold {
            signal[i] = signal[i+1];
        }
    }

    signal
}

#[allow(dead_code)]
fn nuttall_window(signal: Vec<f32>) -> Vec<f32> {
    // NOTE implements windowing to cut down on "glitched" in the
    // final fft output
    //
    // Uses the Nuttall window approximation as defined on
    // the Wikipedia page for window functions.
    let len = signal.len();
    signal.into_iter().enumerate().map(|(n, x)| {
        let n = n as f32;
        let len = len as f32;
        let a0 = 0.355768;
        let a1 = 0.487396;
        let a2 = 0.144232;
        let a3 = 0.012604;
        let res = a0 - a1*(2.0*std::f32::consts::PI*n/(len)).cos()
            + a2*(4.0*std::f32::consts::PI*n/(len)).cos()
            - a3*(6.0*std::f32::consts::PI*n/(len)).cos();

        x*res
    }).collect::<Vec<f32>>()
}

/*
#[allow(dead_code)]
fn dolph_tchebyshev_window(signal: Vec<f32>) -> Vec<f32> {
    // NOTE implements as described in ... TODO
    let len = signal.len();
    signal.into_iter().enumerate().map(|(n, x)| {
        // TODO
        0.0f32
    }).collect()
}
*/

#[allow(dead_code)]
fn gaussian_window(signal: Vec<f32>) -> Vec<f32> {
    // NOTE implements as described in ... TODO
    let len = signal.len();
    let alpha = 2.5f32;
    signal.into_iter().enumerate().map(|(n, x)| {
        let n = n as i64;
        let len = len as i64;
        let scale = -0.5f32;
        let pos = ((n-(len/2)) as f32).abs()/(len as f32/2.0f32);
        let inner = (alpha*pos).powf(2.0f32);
        x*scale*inner.exp()
    }).collect()
}

#[allow(dead_code)]
fn triangle_window(signal: Vec<f32>) -> Vec<f32> {
    // NOTE implements as described in ... TODO
    let len = signal.len();
    signal.into_iter().enumerate().map(|(n, x)| {
        let res = if n <= len/2 {
            (n as f32) / (len as f32 /2.0)
        } else {
            ((len - n) as f32) / (len as f32 /2.0)
        };

        x*res
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    static INIT_AF: std::sync::Once = std::sync::Once::new();

    // NOTE needs to be called before any tests using arrayfire
    // -- https://stackoverflow.com/a/58006287 (thanks explaining per class test init.)
    #[test]
    fn init_af() {
        INIT_AF.call_once(|| {
            AF::set_backend(AF::Backend::CPU);
            AF::set_device(0);
        });
    }

    #[test]
    fn test_stddev_outlier_removal() {
        let signal = vec!{0.0, 1.0, 0.5, 0.2, 0.7};
        let exp_signal = vec!{0.0, 1.0, 0.5, 0.2, 0.7};

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 14.0;
        let signal = vec!{0.0, 1.0, 0.5, outlier, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};
        let exp_signal = vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300.0;
        let signal = vec!{0.0, 1.0, 0.5, outlier, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};
        let exp_signal = vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300000.0;
        let signal = vec!{0.0, 1.0, 0.5, outlier, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};
        let exp_signal = vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300000.0;
        let signal = vec!{outlier, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};
        let exp_signal = vec!{1.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1};

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300000.0;
        let signal = vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, outlier};
        let exp_signal = vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.2};

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);
    }

    #[test]
    fn test_prep_signals() {
        let num_stars = 3;
        let max_len = 12;

        let stars = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
        };

        let exp_stars = vec!{
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
        };

        let act_stars = prep_signals(&stars[..], WindowFunc::Rectangle);

        assert_eq!((exp_stars, num_stars, max_len), act_stars);

        let num_stars = 3;
        let max_len = 12;

        let stars = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 20.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 20.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 20.1},
        };

        let exp_stars = vec!{
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
            0.0, 1.0, 0.5, 0.7, 0.7, 0.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.2,
        };

        let act_stars = prep_signals(&stars[..], WindowFunc::Rectangle);

        assert_eq!((exp_stars, num_stars, max_len), act_stars);
    }

    #[test]
    fn test_prep_stars_and_af_to_vec() {
        init_af();

        let num_stars = 3;
        let max_len = 12;

        let stars = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
        };

        let exp_stars_1d = vec!{
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
        };

        let exp_stars_2d = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
        };

        let act_stars = prep_signals(&stars[..], WindowFunc::Rectangle).0;
        let act_stars = prep_stars(&act_stars, num_stars, max_len);

        let act_stars_1d = af_to_vec1d(&act_stars);
        let act_stars_2d = af_to_vec2d(&act_stars, max_len);

        assert_eq!(exp_stars_1d, act_stars_1d);
        assert_eq!(exp_stars_2d, act_stars_2d);
    }

    #[test]
    fn test_stars_dc_removal() {
        init_af();

        let num_stars = 3;
        let max_len = 12;

        let stars = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
        };

        let exp_stars = vec!{
            -0.5,  0.5,  0. ,  0.2,  0.2, -0.5,  0. ,  0.3,  0.3,  0.2, -0.3, -0.4,
            -0.5,  0.5,  0. ,  0.2,  0.2, -0.5,  0. ,  0.3,  0.3,  0.2, -0.3, -0.4,
            -0.5,  0.5,  0. ,  0.2,  0.2, -0.5,  0. ,  0.3,  0.3,  0.2, -0.3, -0.4,
        };

        let act_stars = prep_signals(&stars[..], WindowFunc::Rectangle).0;
        let act_stars = prep_stars(&act_stars, num_stars, max_len);

        let act_stars = stars_dc_removal(&act_stars, max_len);
        let act_stars = af_to_vec1d(&act_stars);

        exp_stars.iter().zip(act_stars.iter()).for_each(|(e, a)| {
            assert_abs_diff_eq!(e, a, epsilon = std::f32::EPSILON);
        });
    }

    #[test]
    fn test_stars_fft() {
        init_af();

        let num_stars = 1;
        let max_len = 8;

        let stars = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8},
        };

        let exp_stars_1d: Vec<Complex<f32>> = vec!{
            Complex::new(4.2, 0.0), Complex::new(0.07781746, -0.6363961),
            Complex::new(-0.3, 0.5), Complex::new(-1.47781746, -0.6363961),

            Complex::new(4.2, 0.0), Complex::new(0.07781746, -0.6363961),
            Complex::new(-0.3, 0.5), Complex::new(-1.47781746, -0.6363961),

            Complex::new(4.2, 0.0), Complex::new(0.07781746, -0.6363961),
            Complex::new(-0.3, 0.5), Complex::new(-1.47781746, -0.6363961),
        };

        let act_stars = prep_signals(&stars[..], WindowFunc::Rectangle).0;
        let act_stars = prep_stars(&act_stars, num_stars, max_len);
        let act_stars = stars_fft(&act_stars, 8, 8/2 - 1);
        let act_stars_1d = af_to_vec1d(&act_stars);

        exp_stars_1d.iter().zip(act_stars_1d.iter()).for_each(|(e, a)| {
            assert_abs_diff_eq!(e.re, a.re, epsilon = std::f32::EPSILON);
            assert_abs_diff_eq!(e.im, a.im, epsilon = std::f32::EPSILON);
        });

        let num_stars = 1;
        let max_len = 9;

        let stars = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.9},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8},
        };

        let exp_stars_1d: Vec<Complex<f32>> = vec!{
            Complex::new(5.1, 0.0), Complex::new(0.42344224, -0.18145562),
            Complex::new(-0.95543776, 0.62728168), Complex::new(-0.75, -0.95262794),
            Complex::new(-1.26800448, 0.28912205),

            Complex::new(5.1, 0.0), Complex::new(0.42344224, -0.18145562),
            Complex::new(-0.95543776, 0.62728168), Complex::new(-0.75, -0.95262794),
            Complex::new(-1.26800448, 0.28912205),

            Complex::new(5.1, 0.0), Complex::new(0.42344224, -0.18145562),
            Complex::new(-0.95543776, 0.62728168), Complex::new(-0.75, -0.95262794),
            Complex::new(-1.26800448, 0.28912205),
        };

        let act_stars = prep_signals(&stars[..], WindowFunc::Rectangle).0;
        let act_stars = prep_stars(&act_stars, num_stars, max_len);
        let act_stars = stars_fft(&act_stars, 9, (9-1)/2);
        let act_stars_1d = af_to_vec1d(&act_stars);

        exp_stars_1d.iter().zip(act_stars_1d.iter()).for_each(|(e, a)| {
            assert_abs_diff_eq!(e.re, a.re, epsilon = 0.001);
            assert_abs_diff_eq!(e.im, a.im, epsilon = 0.001);
        });
    }
}
