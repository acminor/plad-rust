use num::Complex;

use std::sync::Mutex;
use std::collections::HashMap;

use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;

use ring_buffer::RingBuffer;

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
            //signal = stddev_outlier_removal(signal);
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

/// Normalizes stars to have their minimum value at zero.
///
/// Algorithm: Subtract min from signal.
/// - Raises or lowers DC depending on if signal minimum is above or below zero.
pub fn stars_norm_at_zero(stars: &AF_Array<f32>, signal_max_len: usize) -> AF_Array<f32> {
    let star_mins = AF::min(
        stars,
        0,
    );

    let stars_adjust = AF::tile(
        &star_mins,
        AF_Dim4::new(
            &[signal_max_len as u64, 1, 1, 1]
        ),
    );

    AF::sub(stars, &stars_adjust, false)
}

struct HistoricalMeanEntry {
    prev_sum: f32,
    prev_means: RingBuffer<f32>,
    has_finished_startup: bool,
    next_point: usize,
}

impl HistoricalMeanEntry {
    fn new(capacity: usize) -> HistoricalMeanEntry {
        HistoricalMeanEntry {
            prev_sum: 0.0,
            prev_means: RingBuffer::with_capacity(capacity),
            has_finished_startup: false,
            next_point: 0,
        }
    }
}

lazy_static!{
    static ref historical_means_global: Mutex<HashMap<String, HistoricalMeanEntry>>
        = Mutex::new(HashMap::new());
}

/// Keeps track of historical means and updates stars accordingly
///
/// current_time and min/max_time in number of sample time increments
///
/// Algorithm:
/// 1. Calculate current window means
/// 2. Add current window means to historical means
/// 3. Reduce historical means
///    - min/max_time is how many time periods must have passed for the data to be considered
///    - we only want to consider older times not newer to get true mean while event is occuring
///    - for the first (time) data points, consider all points in mean estimation
/// 4. Subtract historical means from current window means
///
/// NOTE: min/max_time should not change throughout the run
pub fn stars_historical_mean_removal(stars: &AF_Array<f32>, star_names: &[String],
    //stars: &[Vec<f32>], star_names: &[String],
                                     signal_max_len: usize, min_time: usize,
                                     max_time: usize, current_time: usize) -> AF_Array<f32> {
    // Borrow for WHOLE function as inner_product is only called in a single threaded fashion
    let mut historical_means = historical_means_global.lock().unwrap();

    let num_stars = star_names.len();

    let stars_means = AF::mean(
        stars,
        0,
    );

    let stars_means = af_to_vec1d(&stars_means);
    println!("New Means: {:?}", stars_means);

    /*
    let stars_means = stars
        .iter()
        .map(|star| {
            let mean = star
                .iter()
                .sum();

            mean / star.len()
        })
        .collect::<Vec<f32>>();
    */

    star_names
        .iter()
        .zip(stars_means.iter())
        .for_each(|(name, mean)| {
            if !historical_means.contains_key(name) {
                historical_means.insert(name.to_string(),
                                        HistoricalMeanEntry::new(max_time+min_time));
            }

            let mut data = historical_means
                .get_mut(name)
                .expect("historical mean removal issue with get_mut (shouldn't happen)");

            data.prev_means.push(*mean);

            if data.next_point < max_time+min_time {
                data.next_point += 1;
            }
        });

    let stars_means = star_names
        .iter()
        .map(|name| {
            let mut data = historical_means
                .get_mut(name)
                .expect("historical mean removal issue with get (shouldn't happen)");

            let mut adjustment_factor = 0.0;

            // makes sure the historical mean window is kept small
            // - subtract min_time to
            // -- i.e. that the data averaged is max_time window long
            // - total storage space = max_time + min_time
            //if data.prev_means.len() as i64 - min_time as i64 > max_time as i64 {
            //    // remove oldest
            //    adjustment_factor -= data.prev_means.remove(0);
            //}

            // data only consists of historic data (operate normally)
            if data.prev_means.len() >= min_time && data.has_finished_startup {
                adjustment_factor +=
                    data.prev_means.get_relative(data.next_point).unwrap(); // get current point
            } // data only consists of non-historic startup data; except first point (leave startup)
            else if data.prev_means.len() >= min_time {
                //data.next_point = 1;
                data.prev_sum = 0.0;
                data.has_finished_startup = true;

                adjustment_factor +=
                    data.prev_means.get_relative(
                        data.next_point).unwrap(); // get current point
            } // data only consists of non-historic startup data (operate startup)
            else {
               adjustment_factor +=
                    data.prev_means.get_relative(
                        data.prev_means.len() - data.next_point).unwrap(); // get current point
                   //data.prev_means.get_relative(data.next_point - 1).unwrap(); // get current point
            }

            println!("Last clause with np {}, cp {}, af {}, new_mean {}, dp {}",
                     data.next_point, data.next_point - 1, adjustment_factor,
                     (data.prev_sum + adjustment_factor)/data.next_point as f32,
                     data.prev_means.get_relative(data.prev_means.len() - (data.next_point))
                     .unwrap());
            println!("Buffer: {:?}", data.prev_means);
            println!("Buffer np: {:?}", data.prev_means.get_relative(data.next_point));
            println!("Buffer cp: {:?}", data.prev_means.get_relative(data.next_point - 1));
            println!("Buffer cp-1: {:?}", data.prev_means.get_relative(data.next_point - 2));

            data.prev_sum += adjustment_factor;

            if data.has_finished_startup {
                data.prev_sum / (data.next_point - (min_time - 1)) as f32 // here next_point can represent window length
            } else {
                data.prev_sum / data.next_point as f32 // here next_point can represent window length
            }
        }).collect::<Vec<f32>>();

    println!("Means {:?}", stars_means);

    //let stars = prep_stars(&stars[..], num_stars, signal_max_len);

    let stars_means = AF_Array::new(
        &stars_means[..],
        AF_Dim4::new(&[1 as u64, num_stars as u64, 1, 1]),
    );
    let stars_means = AF::tile(
        &stars_means,
        AF_Dim4::new(
            &[signal_max_len as u64, 1, 1, 1]
        ),
    );

    //AF::sub(&stars, &stars_means, false)
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
    fn test_stars_historical_mean_removal() {
        init_af();

        let star_windows: Vec<Vec<f32>> = vec!{
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 1.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.583
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 2.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.667
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 3.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.750
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 4.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.833
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 5.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.917
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 6.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.000
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 7.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.083
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 8.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.167
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 9.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.250
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 1.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.625
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 2.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.708
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 3.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.792
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 4.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.875
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 5.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.958
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 6.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.042
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 7.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.125
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 8.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.208
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 9.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 1.292
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 1.7, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.642
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 2.7, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.725
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 3.7, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1}, // mean: 0.808
        };

        let star_means: Vec<f32> = vec!{
            0.583, 0.667, 0.750, 0.833, 0.917, 1.000, 1.083, 1.167, 1.250,
            0.625, 0.708, 0.792, 0.875, 0.958, 1.042, 1.125, 1.208, 1.292,
            0.642, 0.725, 0.808
        };

        let exp_star_windows: Vec<Vec<f32>> = vec!{
            // first stage (average of averages)
            vec!{-0.583,  0.417, -0.083,  0.117,  0.117,  0.417, -0.083,  0.217,
                 0.217,  0.117, -0.383, -0.483}, // mean: 0.583
            vec!{-0.625,  0.375, -0.125,  0.075,  0.075,  1.375, -0.125,  0.175,
                 0.175,  0.075, -0.425, -0.525}, // mean: 0.667
            vec!{-0.66666667,  0.33333333, -0.16666667,  0.03333333,  0.03333333,
                 2.33333333, -0.16666667,  0.13333333,  0.13333333,  0.03333333,
                 -0.46666667, -0.56666667}, // mean: 0.750
            vec!{-0.70825,  0.29175, -0.20825, -0.00825, -0.00825,  3.29175,
                 -0.20825,  0.09175,  0.09175, -0.00825, -0.50825, -0.60825}, // mean: 0.833
            vec!{-0.75,  0.25, -0.25, -0.05, -0.05,  4.25, -0.25,  0.05,  0.05,
                 -0.05, -0.55, -0.65}, // mean: 0.917
            vec!{-0.79166667,  0.20833333, -0.29166667, -0.09166667, -0.09166667,
                 5.20833333, -0.29166667,  0.00833333,  0.00833333, -0.09166667,
                 -0.59166667, -0.69166667}, // mean: 1.000
            vec!{-0.83328571,  0.16671429, -0.33328571, -0.13328571, -0.13328571,
                 6.16671429, -0.33328571, -0.03328571, -0.03328571, -0.13328571,
                 -0.63328571, -0.73328571}, // mean: 1.083
            vec!{-0.875,  0.125, -0.375, -0.175, -0.175,  7.125, -0.375, -0.075,
                 -0.075, -0.175, -0.675, -0.775}, // mean: 1.167
            vec!{-0.91666667,  0.08333333, -0.41666667, -0.21666667, -0.21666667,
                 8.08333333, -0.41666667, -0.11666667, -0.11666667, -0.21666667,
                 -0.71666667, -0.81666667}, // mean: 1.250
            vec!{-0.8875,  0.1125, -0.3875, -0.1875, -0.1875,  0.6125, -0.3875,
                 -0.0875, -0.0875, -0.1875, -0.6875, -0.7875}, // mean: 0.625
            // transition to second stage (restart average of averages)
            vec!{-0.583,  0.417, -0.083,  0.117,  0.117,  1.917, -0.083,  0.217,
                 0.217,  0.117, -0.383, -0.483}, // mean: 0.708
            vec!{-0.625,  0.375, -0.125,  0.075,  0.075,  2.875, -0.125,  0.175,
                 0.175,  0.075, -0.425, -0.525}, // mean: 0.792
            vec!{-0.66666667,  0.33333333, -0.16666667,  0.03333333,  0.03333333,
                 3.83333333, -0.16666667,  0.13333333,  0.13333333,  0.03333333,
                 -0.46666667, -0.56666667}, // mean: 0.875
            vec!{-0.70825,  0.29175, -0.20825, -0.00825, -0.00825,  4.79175,
                 -0.20825,  0.09175,  0.09175, -0.00825, -0.50825, -0.60825}, // mean: 0.958
            vec!{-0.75,  0.25, -0.25, -0.05, -0.05,  5.75, -0.25,  0.05,  0.05,
                 -0.05, -0.55, -0.65}, // mean: 1.042
            vec!{-0.79166667,  0.20833333, -0.29166667, -0.09166667, -0.09166667,
                 6.70833333, -0.29166667,  0.00833333,  0.00833333, -0.09166667,
                 -0.59166667, -0.69166667}, // mean: 1.125
            vec!{-0.83328571,  0.16671429, -0.33328571, -0.13328571, -0.13328571,
                 7.66671429, -0.33328571, -0.03328571, -0.03328571, -0.13328571,
                 -0.63328571, -0.73328571}, // mean: 1.208
            vec!{-0.875,  0.125, -0.375, -0.175, -0.175,  8.625, -0.375, -0.075,
                 -0.075, -0.175, -0.675, -0.775}, // mean: 1.292
            vec!{-0.91666667,  0.08333333, -0.41666667, -0.21666667, -0.21666667,
                 0.78333333, -0.41666667, -0.11666667, -0.11666667, -0.21666667,
                 -0.71666667, -0.81666667}, // mean: 0.642
            vec!{-0.8875,  0.1125, -0.3875, -0.1875, -0.1875,  1.8125, -0.3875,
                 -0.0875, -0.0875, -0.1875, -0.6875, -0.7875}, // mean: 0.725
            // cross transition point and start shifting out values
            // (i.e. don't include 0.583 in calculation)
            vec!{-0.9,  0.1, -0.4, -0.2, -0.2,  2.8, -0.4, -0.1, -0.1, -0.2, -0.7,
                 -0.8}, // mean: 0.808
        };

        let max_len = 12;
        let num_windows = 21;
        let num_stars = 1;
        for i in 0..num_windows {
            println!("Checking star window {}", i);
            println!("act stars {:?}", star_windows[i].clone());
            let act_stars = prep_signals(&[star_windows[i].clone()], WindowFunc::Rectangle).0;
            println!("act stars {:?}", act_stars);
            let act_stars = prep_stars(&act_stars, num_stars, max_len);

            let act_stars = stars_historical_mean_removal(&act_stars, &["blah".to_string()],
                                                          max_len, 11, 10, i);
            let act_stars = af_to_vec1d(&act_stars);

            // all of my calculations were done to 3 significant places and then rounded
            // thus, we use 0.001 as the epsilon
            exp_star_windows[i].iter().zip(act_stars.iter()).for_each(|(e, a)| {
                assert_abs_diff_eq!(e, a, epsilon = 0.001);
            });
        }
    }

    #[test]
    fn test_stars_norm_at_zero() {
        init_af();

        let num_stars = 3;
        let max_len = 12;

        let stars = vec!{
            vec!{0.1, 1.0, 0.5, 0.7, 0.7, 0.1, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
            vec!{-0.3, 1.0, 0.5, 0.7, 0.7, -0.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1},
        };

        let exp_stars = vec!{
            0.0, 0.9, 0.4, 0.6, 0.6, 0.0, 0.4, 0.7, 0.7, 0.6, 0.1, 0.0,
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
            0.2, 1.5, 1.0, 1.2, 1.2, 0.0, 1.0, 1.3, 1.3, 1.2, 0.7, 0.6,
        };

        let act_stars = prep_signals(&stars[..], WindowFunc::Rectangle).0;
        let act_stars = prep_stars(&act_stars, num_stars, max_len);

        let act_stars = stars_norm_at_zero(&act_stars, max_len);
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
