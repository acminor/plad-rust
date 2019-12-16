use num::Complex;

use std::collections::HashMap;
use std::sync::Mutex;

use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;

use crate::cyclic_queue::{CyclicQueue, CyclicQueueInterface};

arg_enum! {
    #[derive(Clone, Copy)]
    #[allow(dead_code)]
    pub enum WindowFunc {
        Nuttall,
        Rectangle,
        Triangle,
        Gaussian,
    }
}

pub fn outlier_removal_stars(signals: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
    signals
        .into_iter()
        .map(|signal| stddev_outlier_removal(signal))
        .collect::<Vec<Vec<f32>>>()
}

pub fn window_signals(
    signals: Vec<Vec<f32>>,
    window_type: WindowFunc,
) -> Vec<Vec<f32>> {
    let window_func = match window_type {
        WindowFunc::Nuttall => nuttall_window,
        WindowFunc::Triangle => triangle_window,
        WindowFunc::Gaussian => gaussian_window,
        WindowFunc::Rectangle => |arr| arr,
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
        .into_iter()
        .map(|signal| window_func(signal))
        .collect::<Vec<Vec<f32>>>();

    signals
}

pub fn stars_to_af(signals: Vec<Vec<f32>>) -> (AF_Array<f32>, usize, usize) {
    let num_stars = signals.len();
    let signal_max_len = signals
        .iter()
        .map(|signal| signal.len())
        .max()
        .expect("Problem getting the max signal length.");

    let signals = signals
        .into_iter()
        .flat_map(|signal| {
            let len = signal.len();
            signal
                .into_iter()
                .chain(std::iter::repeat(0.0f32).take(signal_max_len - len))
        })
        .collect::<Vec<f32>>();

    let stars = AF_Array::new(
        &signals[..],
        AF_Dim4::new(&[signal_max_len as u64, num_stars as u64, 1, 1]),
    );

    (stars, num_stars, signal_max_len)
}

pub fn af_to_vec1d<T>(arr: &AF_Array<T>) -> Vec<T>
where
    T: AF::HasAfEnum + std::clone::Clone + std::default::Default,
{
    let mut temp: Vec<T> = Vec::new();
    temp.resize(arr.elements(), Default::default());
    arr.lock();
    arr.host(&mut temp);
    arr.unlock();

    temp
}

#[allow(dead_code)]
pub fn af_to_vec2d<T>(arr: &AF_Array<T>, dim_1_len: usize) -> Vec<Vec<T>>
where
    T: AF::HasAfEnum + std::clone::Clone + std::default::Default,
{
    let mut temp: Vec<T> = Vec::new();
    temp.resize(arr.elements(), Default::default());
    arr.lock();
    arr.host(&mut temp);
    arr.unlock();

    temp.chunks(dim_1_len)
        .map(|chunk| Vec::from(chunk))
        .collect()
}

pub fn stars_dc_removal(stars: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
    // NOTE Remove DC constant of template to focus on signal
    //      - This is very important and will lead to false
    //        detection or searching for the wrong signal
    let stars_means = means(&stars);

    subtract_means(stars, &stars_means)
}

/// Normalizes stars to have their minimum value at zero.
///
/// Algorithm: Subtract min from signal.
/// - Raises or lowers DC depending on if signal minimum is above or below zero.
pub fn stars_norm_at_zero(stars: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
    let star_mins = mins(&stars);

    subtract_means(stars, &star_mins)
}

fn means(signals: &[Vec<f32>]) -> Vec<f32> {
    signals
        .iter()
        .map(|signal| {
            let mean: f32 = signal.iter().sum();

            mean / signal.len() as f32
        })
        .collect::<Vec<f32>>()
}

#[allow(unused)]
fn maxes(signals: &[Vec<f32>]) -> Vec<f32> {
    let maxes =
        signals
            .iter()
            .map(|star| {
                star.iter().fold(std::f32::MIN, |acc, &val| {
                    if val > acc {
                        val
                    } else {
                        acc
                    }
                })
            })
            .collect::<Vec<f32>>();

    maxes
}

fn mins(signals: &[Vec<f32>]) -> Vec<f32> {
    let mins =
        signals
            .iter()
            .map(|star| {
                star.iter().fold(std::f32::MAX, |acc, &val| {
                    if val < acc {
                        val
                    } else {
                        acc
                    }
                })
            })
            .collect::<Vec<f32>>();

    mins
}

fn subtract_means(signals: Vec<Vec<f32>>, means: &Vec<f32>) -> Vec<Vec<f32>> {
    signals
        .into_iter()
        .zip(means.iter())
        .map(|(signal, mean)| {
            signal
                .into_iter()
                .map(|val| val - mean)
                .collect::<Vec<f32>>()
        })
        .collect()
}

#[derive(Clone, Copy)]
enum HistoricalMeanEntryStage {
    /// data only consists of non-historic startup data (operate startup)
    Startup,
    PostStartupWarmup,
    /// data only consists of historic data (operate normally)
    PostStartup,
}

struct HistoricalMeanEntry {
    prev_sum: f32,
    prev_means: CyclicQueue<f32>,
    stage: HistoricalMeanEntryStage,
    next_point: usize,
    /// for use in adjusting mean by current for warmup stage
    current_mean: f32,
    /// for use in adjusting mean by current for warmup stage
    /// - percentage of mean that should be b/c of current mean
    current_mean_split: f32,
    /// for use in transitioning to the final stage
    counter: usize,
}

impl HistoricalMeanEntry {
    fn new(capacity: usize) -> HistoricalMeanEntry {
        HistoricalMeanEntry {
            prev_sum: 0.0,
            prev_means: CyclicQueue::new(capacity),
            stage: HistoricalMeanEntryStage::Startup,
            next_point: 0,
            current_mean: 0.0,
            current_mean_split: 0.0,
            counter: 0,
        }
    }
}

lazy_static! {
    static ref HISTORICAL_MEANS_GLOBAL: Mutex<HashMap<String, HistoricalMeanEntry>> =
        Mutex::new(HashMap::new());
}

#[derive(PartialEq)]
pub enum HistoricalMeanRunType {
    Fast,                    // fast summation O(1)
    Natural, // natural is the brute force summation algorithm O(N)
    CheckFastAgainstNatural, // panics if fast and natural differ (after setup phase)
}

/// Keeps track of historical means and updates stars accordingly
///
/// current_time and min/max_time in number of sample time increments
/// -> for now current_time is unused and this function should be used
///    with delta_skip = 1 for current_time like functionality
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
pub fn stars_historical_mean_removal(
    stars: Vec<Vec<f32>>,
    star_names: &[String],
    min_time: usize,
    delta_time: usize,
    _current_time: usize,
    run_type: HistoricalMeanRunType,
) -> Vec<Vec<f32>> {
    // Borrow for WHOLE function as inner_product is only called in a single threaded fashion
    let mut historical_means = HISTORICAL_MEANS_GLOBAL.lock().unwrap();

    //let num_stars = star_names.len();

    let stars_means = means(&stars);

    let mut init_adjustments: HashMap<String, f32> = HashMap::new();

    star_names
        .iter()
        .zip(stars_means.iter())
        .for_each(|(name, mean)| {
            if !historical_means.contains_key(name) {
                historical_means.insert(
                    name.to_string(),
                    HistoricalMeanEntry::new(delta_time + min_time),
                );
            }

            let mut data = historical_means.get_mut(name).expect(
                "historical mean removal issue with get_mut (shouldn't happen)",
            );

            if data.next_point < delta_time {
                //min_time {
                data.next_point += 1;
            }

            let prev_val = data.prev_means.push(*mean);

            let prev_val = prev_val.map(|val| -val).unwrap_or(0.0);

            init_adjustments.insert(name.to_string(), prev_val);
        });

    let stars_means = star_names
        .iter()
        .map(|name| {
            let mut data = historical_means
                .get_mut(name)
                .expect("historical mean removal issue with get (shouldn't happen)");

            // make sure the removed value is considered as negative (subtraction)
            let mut adjustment_factor = init_adjustments[name];
            //let org_af = init_adjustments[name];

            // makes sure the historical mean window is kept small
            // - subtract min_time to
            // -- i.e. that the data averaged is max_time window long
            // - total storage space = max_time + min_time
            //if data.prev_means.len() as i64 - min_time as i64 > max_time as i64 {
            //    // remove oldest
            //    adjustment_factor -= data.prev_means.remove(0);
            //}

            match data.stage {
                HistoricalMeanEntryStage::Startup => {
                    // Transition to POST_STARTUP
                    if data.prev_means.len() > min_time {
                        // NOTE could be useful in testing fix for grouping issue
                        //     (see FIXME XXX in detector.rs)
                        //println!("{} trans at {}", name, data.next_point);
                        data.next_point = 1;
                        data.prev_sum = 0.0;
                        data.stage = HistoricalMeanEntryStage::PostStartupWarmup;

                        adjustment_factor += // get current point
                            data.prev_means.get_relative(data.next_point - 1).unwrap();
                    } else {
                        // NOTE: for startup for each window only use current windows average
                        // - helps to avoid issues where upward/downward slopes
                        //   arbitrarily increase filter output (has been observed)
                        // multiply by data.next_point b/c dividing that for average
                        // - ensures only using current average while not adding additional if-stmt
                        data.prev_sum = data.next_point as f32 *
                            data.prev_means.get_relative(data.next_point - 1).unwrap();
                    }
                }
                HistoricalMeanEntryStage::PostStartupWarmup => {
                    // ensures at least k previous means are averaged to get results
                    // before switching to "true" averaging
                    // NOTE: assumes that max_duration is greater than 10 ???
                    let k = 10;
                    data.counter += 1;

                    // linear scale down from 100% -> 0% over k length
                    // - NOTE: abs is to avoid floating accuracy from making
                    //         current_mean_split < 0.0 at count=k
                    data.current_mean_split = (data.counter as f32)*(-1.0/k as f32) + 1.0;
                    data.current_mean = *data.prev_means.get_back().unwrap();

                    if data.counter ==  k {
                        data.stage = HistoricalMeanEntryStage::PostStartup;
                        data.current_mean_split = 0.0;
                        data.current_mean = 0.0;
                    }

                    adjustment_factor +=  // get current point
                        data.prev_means.get_relative(data.next_point - 1).unwrap();
                }
                HistoricalMeanEntryStage::PostStartup => {
                    adjustment_factor += // get current point
                        data.prev_means.get_relative(data.next_point - 1).unwrap();
                }
            }

            match run_type {
                HistoricalMeanRunType::Fast => {
                    data.prev_sum += adjustment_factor;
                }
                HistoricalMeanRunType::Natural |
                HistoricalMeanRunType::CheckFastAgainstNatural => {
                    let mut prev_sum = 0.0;
                    for i in 0..data.next_point {
                        prev_sum += data.prev_means.get_relative(i).unwrap();
                    }

                    if run_type == HistoricalMeanRunType::CheckFastAgainstNatural {
                        data.prev_sum += adjustment_factor;

                        match data.stage {
                            HistoricalMeanEntryStage::PostStartup |
                            HistoricalMeanEntryStage::PostStartupWarmup => {
                                assert_abs_diff_eq!(prev_sum, data.prev_sum, epsilon=0.001);
                            }
                            _ => {}
                        }
                    }

                    data.prev_sum = prev_sum;
                }
            }

            // here next_point can represent window length
            let normalized_mean_a = (data.prev_sum/data.next_point as f32)
                * (1.0-data.current_mean_split);
            let normalized_mean_b = data.current_mean*data.current_mean_split;

            normalized_mean_a + normalized_mean_b
        }).collect::<Vec<f32>>();

    subtract_means(stars, &stars_means)
}

/*
struct MMHistoricalMeanEntry {
    average_min: f32,
    min_min: f32,
    average_max: f32,
    max_max: f32,
    mins: CyclicQueue<f32>,
    maxes: CyclicQueue<f32>,
}

impl MMHistoricalMeanEntry {
    fn new(cap: usize) -> MMHistoricalMeanEntry {
        MMHistoricalMeanEntry {
            min_min: std::f32::MAX,
            max_max: std::f32::MIN,
            average_min: std::f32::MAX,
            average_max: std::f32::MIN,
            mins: CyclicQueue::new(cap),
            maxes: CyclicQueue::new(cap),
        }
    }
}

lazy_static!{
    static ref historical_min_max_global: Mutex<HashMap<String, MMHistoricalMeanEntry>>
        = Mutex::new(HashMap::new());
}

pub fn stars_min_max_historical_mean_removal(stars: Vec<Vec<f32>>, star_names: &[String],
                                             min_time: usize,
                                             delta_time: usize, current_time: usize)
                                             -> Vec<Vec<f32>> {
    // Borrow for WHOLE function as inner_product is only called in a single threaded fashion
    let mut historical_min_max = historical_min_max_global.lock().unwrap();

    let num_stars = star_names.len();
    let stars_mins = mins(&stars);
    let stars_maxes = maxes(&stars);

    star_names
        .iter()
        .zip(stars_mins
             .iter()
             .zip(stars_maxes.iter())
        )
        .for_each(|(name, (min, max))| {
            if !historical_min_max.contains_key(name) {
                historical_min_max.insert(name.to_string(),
                                          MMHistoricalMeanEntry::new());
            }

            let mut data = historical_min_max
                .get_mut(name)
                .expect("historical mean removal issue with get_mut (shouldn't happen)");

            if *min < data.min {
                data.min = *min;
            }

            if *max > data.max {
                data.max = *max;
            }
        });

    let stars_means = star_names
        .iter()
        .map(|name| {
            let mut data = historical_min_max
                .get_mut(name)
                .expect("historical mean removal issue with get (shouldn't happen)");

            data.min + (data.max - data.min) / 2.0
        }).collect::<Vec<f32>>();

    subtract_means(stars, &stars_means)
}
*/

pub fn stars_fft(
    stars: &AF_Array<f32>,
    fft_len: usize,
    fft_half_len: usize,
) -> AF_Array<Complex<f32>> {
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
    let stddev: f32 = signal.iter().map(|x| (x - mean).powf(2.0)).sum();
    let stddev = stddev / (len - 1) as f32;
    let stddev = stddev.sqrt();

    let threshold = mean + 3.0 * stddev;

    // NOTE assumes that the signal is at least 2 length wide
    // and that their is not more that two errors in a row
    // NOTE last case extracted here to remove redundant if
    // statement in the for loop below
    if signal[len - 1] > threshold {
        signal[len - 1] = signal[len - 2];
    }

    for i in 0..len - 1 {
        // NOTE for now does not handle more than two errors in a row
        // - it will back propagate these errors
        if signal[i] > threshold {
            signal[i] = signal[i + 1];
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
    signal
        .into_iter()
        .enumerate()
        .map(|(n, x)| {
            let n = n as f32;
            let len = len as f32;
            let a0 = 0.355768;
            let a1 = 0.487396;
            let a2 = 0.144232;
            let a3 = 0.012604;
            let res = a0 - a1 * (2.0 * std::f32::consts::PI * n / (len)).cos()
                + a2 * (4.0 * std::f32::consts::PI * n / (len)).cos()
                - a3 * (6.0 * std::f32::consts::PI * n / (len)).cos();

            x * res
        })
        .collect::<Vec<f32>>()
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
    // TODO validate
    let len = signal.len();
    let alpha = 2.5f32;
    signal
        .into_iter()
        .enumerate()
        .map(|(n, x)| {
            let n = n as i64;
            let len = len as i64;
            let scale = -0.5f32;
            //let pos = ((n - (len / 2)) as f32).abs() / (len as f32 / 2.0f32);
            let pos = (n as f32 - (len as f32 / 2.0)) / (len as f32 / 2.0f32);
            let inner = (alpha * pos).powf(2.0f32);
            x * (scale * inner).exp()
        })
        .collect()
}

#[allow(dead_code)]
fn triangle_window(signal: Vec<f32>) -> Vec<f32> {
    // NOTE implements as described in ... TODO
    let len = signal.len();
    signal
        .into_iter()
        .enumerate()
        .map(|(n, x)| {
            let res = if n <= len / 2 {
                (n as f32) / (len as f32 / 2.0)
            } else {
                ((len - n) as f32) / (len as f32 / 2.0)
            };

            x * res
        })
        .collect()
}

fn kaiser_bessel_window(signal: Vec<f32>) -> Vec<f32> {
    // NOTE implements as described in ... TODO
    let approx_inif = 1000;

    let alpha = 3.0;

    let e = std::f32::consts::E;
    let pi = std::f32::consts::PI;

    let modified_bessel = |x: f32| {
        (0..approx_inif)
            .map(|v| v as f32)
            .fold(0.0, |res: f32, k: f32| {
                let num = (x/2.0).powf(k);
                // Stirling approx to factorial from Knuth Book I TODO
                let denom = (2.0*pi*k).sqrt() * (k/e).powf(k);

                res + (num/denom).powf(2.0)
            })
    };

    let len = signal.len();
    signal
        .into_iter()
        .enumerate()
        .map(|(n, x)| {
            let n = n - len/2;
            let num = modified_bessel(pi*alpha*(1.0 - (n as f32/(len as f32/2.0)).powf(2.0)));
            let denom = modified_bessel(pi*alpha);

            x*num/denom
        })
        .collect()
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
        let signal = vec![0.0, 1.0, 0.5, 0.2, 0.7];
        let exp_signal = vec![0.0, 1.0, 0.5, 0.2, 0.7];

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 14.0;
        let signal = vec![
            0.0, 1.0, 0.5, outlier, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
        ];
        let exp_signal =
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1];

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300.0;
        let signal = vec![
            0.0, 1.0, 0.5, outlier, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
        ];
        let exp_signal =
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1];

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300000.0;
        let signal = vec![
            0.0, 1.0, 0.5, outlier, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
        ];
        let exp_signal =
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1];

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300000.0;
        let signal = vec![
            outlier, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
        ];
        let exp_signal =
            vec![1.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1];

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);

        let outlier = 300000.0;
        let signal = vec![
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, outlier,
        ];
        let exp_signal =
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.2];

        let act_signal = stddev_outlier_removal(signal);
        assert_eq!(exp_signal, act_signal);
    }

    #[test]
    fn test_window_signals() {
        let num_stars = 3;
        let max_len = 12;

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let exp_stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let act_stars = window_signals(stars, WindowFunc::Rectangle);

        assert_eq!(exp_stars, act_stars);

        let num_stars = 3;
        let max_len = 12;

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 20.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 20.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 20.1],
        ];

        let exp_stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 20.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 20.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 20.1],
        ];

        let act_stars = window_signals(stars, WindowFunc::Rectangle);

        assert_eq!(exp_stars, act_stars);
    }

    #[test]
    fn test_outlier_removal_stars() {
        let num_stars = 3;
        let max_len = 12;

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let exp_stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let act_stars = outlier_removal_stars(stars);

        assert_eq!(exp_stars, act_stars);

        let num_stars = 3;
        let max_len = 12;

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 20.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 20.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 20.1],
        ];

        let exp_stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.2],
        ];

        let act_stars = outlier_removal_stars(stars);

        assert_eq!(exp_stars, act_stars);
    }

    #[test]
    fn test_stars_to_af_and_af_to_vec() {
        init_af();

        let num_stars = 3;
        let max_len = 12;

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let exp_stars_1d = vec![
            0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1, 0.0,
            1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1, 0.0, 1.0,
            0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1,
        ];

        let exp_stars_2d = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let (act_stars, num_stars, max_len) = stars_to_af(stars);

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

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let exp_stars = vec![
            -0.5, 0.5, 0., 0.2, 0.2, -0.5, 0., 0.3, 0.3, 0.2, -0.3, -0.4, -0.5,
            0.5, 0., 0.2, 0.2, -0.5, 0., 0.3, 0.3, 0.2, -0.3, -0.4, -0.5, 0.5,
            0., 0.2, 0.2, -0.5, 0., 0.3, 0.3, 0.2, -0.3, -0.4,
        ];

        let act_stars = stars_dc_removal(stars);
        let act_stars = act_stars.into_iter().flat_map(|star| star.into_iter());

        exp_stars.into_iter().zip(act_stars).for_each(|(e, a)| {
            assert_abs_diff_eq!(e, a, epsilon = std::f32::EPSILON);
        });
    }

    #[test]
    fn test_stars_historical_mean_removal() {
        init_af();

        let star_windows: Vec<Vec<f32>> = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 1.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.583
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 2.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.667
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 3.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.750
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 4.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.833
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 5.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.917
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 6.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.000
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 7.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.083
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 8.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.167
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 9.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.250
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 1.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.625
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 2.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.708
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 3.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.792
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 4.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.875
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 5.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.958
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 6.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.042
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 7.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.125
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 8.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.208
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 9.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 1.292
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 1.7, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.642
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 2.7, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.725
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 3.7, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1], // mean: 0.808
        ];

        let star_means: Vec<f32> = vec![
            0.583, 0.667, 0.750, 0.833, 0.917, 1.000, 1.083, 1.167, 1.250,
            0.625, 0.708, 0.792, 0.875, 0.958, 1.042, 1.125, 1.208, 1.292,
            0.642, 0.725, 0.808,
        ];

        let exp_star_windows: Vec<Vec<f32>> = vec![
            // first stage (average of averages)
            vec![
                -0.583, 0.417, -0.083, 0.117, 0.117, 0.417, -0.083, 0.217,
                0.217, 0.117, -0.383, -0.483,
            ], // mean: 0.583
            vec![
                -0.625, 0.375, -0.125, 0.075, 0.075, 1.375, -0.125, 0.175,
                0.175, 0.075, -0.425, -0.525,
            ], // mean: 0.667
            vec![
                -0.66666667,
                0.33333333,
                -0.16666667,
                0.03333333,
                0.03333333,
                2.33333333,
                -0.16666667,
                0.13333333,
                0.13333333,
                0.03333333,
                -0.46666667,
                -0.56666667,
            ], // mean: 0.750
            vec![
                -0.70825, 0.29175, -0.20825, -0.00825, -0.00825, 3.29175,
                -0.20825, 0.09175, 0.09175, -0.00825, -0.50825, -0.60825,
            ], // mean: 0.833
            vec![
                -0.75, 0.25, -0.25, -0.05, -0.05, 4.25, -0.25, 0.05, 0.05,
                -0.05, -0.55, -0.65,
            ], // mean: 0.917
            vec![
                -0.79166667,
                0.20833333,
                -0.29166667,
                -0.09166667,
                -0.09166667,
                5.20833333,
                -0.29166667,
                0.00833333,
                0.00833333,
                -0.09166667,
                -0.59166667,
                -0.69166667,
            ], // mean: 1.000
            vec![
                -0.83328571,
                0.16671429,
                -0.33328571,
                -0.13328571,
                -0.13328571,
                6.16671429,
                -0.33328571,
                -0.03328571,
                -0.03328571,
                -0.13328571,
                -0.63328571,
                -0.73328571,
            ], // mean: 1.083
            vec![
                -0.875, 0.125, -0.375, -0.175, -0.175, 7.125, -0.375, -0.075,
                -0.075, -0.175, -0.675, -0.775,
            ], // mean: 1.167
            vec![
                -0.91666667,
                0.08333333,
                -0.41666667,
                -0.21666667,
                -0.21666667,
                8.08333333,
                -0.41666667,
                -0.11666667,
                -0.11666667,
                -0.21666667,
                -0.71666667,
                -0.81666667,
            ], // mean: 1.250
            vec![
                -0.8875, 0.1125, -0.3875, -0.1875, -0.1875, 0.6125, -0.3875,
                -0.0875, -0.0875, -0.1875, -0.6875, -0.7875,
            ], // mean: 0.625
            // transition to second stage (restart average of averages)
            vec![
                -0.583, 0.417, -0.083, 0.117, 0.117, 1.917, -0.083, 0.217,
                0.217, 0.117, -0.383, -0.483,
            ], // mean: 0.708
            vec![
                -0.625, 0.375, -0.125, 0.075, 0.075, 2.875, -0.125, 0.175,
                0.175, 0.075, -0.425, -0.525,
            ], // mean: 0.792
            vec![
                -0.66666667,
                0.33333333,
                -0.16666667,
                0.03333333,
                0.03333333,
                3.83333333,
                -0.16666667,
                0.13333333,
                0.13333333,
                0.03333333,
                -0.46666667,
                -0.56666667,
            ], // mean: 0.875
            vec![
                -0.70825, 0.29175, -0.20825, -0.00825, -0.00825, 4.79175,
                -0.20825, 0.09175, 0.09175, -0.00825, -0.50825, -0.60825,
            ], // mean: 0.958
            vec![
                -0.75, 0.25, -0.25, -0.05, -0.05, 5.75, -0.25, 0.05, 0.05,
                -0.05, -0.55, -0.65,
            ], // mean: 1.042
            vec![
                -0.79166667,
                0.20833333,
                -0.29166667,
                -0.09166667,
                -0.09166667,
                6.70833333,
                -0.29166667,
                0.00833333,
                0.00833333,
                -0.09166667,
                -0.59166667,
                -0.69166667,
            ], // mean: 1.125
            vec![
                -0.83328571,
                0.16671429,
                -0.33328571,
                -0.13328571,
                -0.13328571,
                7.66671429,
                -0.33328571,
                -0.03328571,
                -0.03328571,
                -0.13328571,
                -0.63328571,
                -0.73328571,
            ], // mean: 1.208
            vec![
                -0.875, 0.125, -0.375, -0.175, -0.175, 8.625, -0.375, -0.075,
                -0.075, -0.175, -0.675, -0.775,
            ], // mean: 1.292
            vec![
                -0.91666667,
                0.08333333,
                -0.41666667,
                -0.21666667,
                -0.21666667,
                0.78333333,
                -0.41666667,
                -0.11666667,
                -0.11666667,
                -0.21666667,
                -0.71666667,
                -0.81666667,
            ], // mean: 0.642
            vec![
                -0.8875, 0.1125, -0.3875, -0.1875, -0.1875, 1.8125, -0.3875,
                -0.0875, -0.0875, -0.1875, -0.6875, -0.7875,
            ], // mean: 0.725
            // cross transition point and start shifting out values
            // (i.e. don't include 0.583 in calculation)
            vec![
                -0.9, 0.1, -0.4, -0.2, -0.2, 2.8, -0.4, -0.1, -0.1, -0.2, -0.7,
                -0.8,
            ], // mean: 0.808
        ];

        let max_len = 12;
        let num_windows = 21;
        let num_stars = 1;
        for i in 0..num_windows {
            println!("Checking star window {}", i);
            let act_stars = stars_historical_mean_removal(
                vec![star_windows[i].clone()],
                &["blah".to_string()],
                10,
                10,
                i,
            );
            let act_stars = act_stars
                .into_iter()
                .flat_map(|star| star.into_iter())
                .collect::<Vec<f32>>();

            // all of my calculations were done to 3 significant places and then rounded
            // thus, we use 0.001 as the epsilon
            exp_star_windows[i].iter().zip(act_stars.iter()).for_each(
                |(e, a)| {
                    assert_abs_diff_eq!(e, a, epsilon = 0.001);
                },
            );
        }
    }

    #[test]
    fn test_stars_norm_at_zero() {
        init_af();

        let num_stars = 3;
        let max_len = 12;

        let stars = vec![
            vec![0.1, 1.0, 0.5, 0.7, 0.7, 0.1, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
            vec![-0.3, 1.0, 0.5, 0.7, 0.7, -0.5, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1],
        ];

        let exp_stars = vec![
            0.0, 0.9, 0.4, 0.6, 0.6, 0.0, 0.4, 0.7, 0.7, 0.6, 0.1, 0.0, 0.0,
            1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.8, 0.7, 0.2, 0.1, 0.2, 1.5,
            1.0, 1.2, 1.2, 0.0, 1.0, 1.3, 1.3, 1.2, 0.7, 0.6,
        ];

        let act_stars = stars_norm_at_zero(stars)
            .into_iter()
            .flat_map(|star| star.into_iter());

        exp_stars.into_iter().zip(act_stars).for_each(|(e, a)| {
            assert_abs_diff_eq!(e, a, epsilon = std::f32::EPSILON);
        });
    }

    #[test]
    fn test_stars_fft() {
        init_af();

        let num_stars = 1;
        let max_len = 8;

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8],
        ];

        let exp_stars_1d: Vec<Complex<f32>> = vec![
            Complex::new(4.2, 0.0),
            Complex::new(0.07781746, -0.6363961),
            Complex::new(-0.3, 0.5),
            Complex::new(-1.47781746, -0.6363961),
            Complex::new(4.2, 0.0),
            Complex::new(0.07781746, -0.6363961),
            Complex::new(-0.3, 0.5),
            Complex::new(-1.47781746, -0.6363961),
            Complex::new(4.2, 0.0),
            Complex::new(0.07781746, -0.6363961),
            Complex::new(-0.3, 0.5),
            Complex::new(-1.47781746, -0.6363961),
        ];

        let (act_stars, _, _) = stars_to_af(stars);
        let act_stars = stars_fft(&act_stars, 8, 8 / 2 - 1);
        let act_stars_1d = af_to_vec1d(&act_stars);

        exp_stars_1d
            .iter()
            .zip(act_stars_1d.iter())
            .for_each(|(e, a)| {
                assert_abs_diff_eq!(e.re, a.re, epsilon = std::f32::EPSILON);
                assert_abs_diff_eq!(e.im, a.im, epsilon = std::f32::EPSILON);
            });

        let num_stars = 1;
        let max_len = 9;

        let stars = vec![
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.9],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.9],
            vec![0.0, 1.0, 0.5, 0.7, 0.7, 0.0, 0.5, 0.8, 0.9],
        ];

        let exp_stars_1d: Vec<Complex<f32>> = vec![
            Complex::new(5.1, 0.0),
            Complex::new(0.42344224, -0.18145562),
            Complex::new(-0.95543776, 0.62728168),
            Complex::new(-0.75, -0.95262794),
            Complex::new(-1.26800448, 0.28912205),
            Complex::new(5.1, 0.0),
            Complex::new(0.42344224, -0.18145562),
            Complex::new(-0.95543776, 0.62728168),
            Complex::new(-0.75, -0.95262794),
            Complex::new(-1.26800448, 0.28912205),
            Complex::new(5.1, 0.0),
            Complex::new(0.42344224, -0.18145562),
            Complex::new(-0.95543776, 0.62728168),
            Complex::new(-0.75, -0.95262794),
            Complex::new(-1.26800448, 0.28912205),
        ];

        let (act_stars, _, _) = stars_to_af(stars);
        let act_stars = stars_fft(&act_stars, 9, (9 - 1) / 2);
        let act_stars_1d = af_to_vec1d(&act_stars);

        exp_stars_1d
            .iter()
            .zip(act_stars_1d.iter())
            .for_each(|(e, a)| {
                assert_abs_diff_eq!(e.re, a.re, epsilon = 0.001);
                assert_abs_diff_eq!(e.im, a.im, epsilon = 0.001);
            });
    }
}
