use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;
use inline_python::python;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;

use crate::template::*;

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

pub fn inner_product(
    templates: &[TemplateGroup],
    signals: &[Vec<f32>],
    // [ ] TODO include in calculations
    //  - ie work on estitmation, etc.
    _snf: f32,
    // [ ] TODO assume always on after template read
    //  - refactor out
    _pre_fft: bool,
    // [ ] TODO refactor into template instant.
    _template_group_len: usize,
    signal_group_len: usize,
) -> Vec<f32> {
    let mut res: Vec<f32> = Vec::new();
    for signals in signals.chunks(signal_group_len) {
        //1) {//signal_group_len) {
        let num_stars = signals.len();
        let signal_max_len = signals
            .iter()
            .map(|signal| signal.len())
            .max()
            .expect("Problem getting the max signal length.");
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
        let signals = &signals
            .iter()
            .cloned()
            .into_iter()
            .flat_map(|mut signal| {
                let len = signal.len();

                let mean: f32 = signal.iter().sum();
                let mean = mean / len as f32;

                // Formula for the corrected sample standard deviation
                // from the Wikipedia article on standard deviation
                let stddev: f32 = signal.iter().map(|x| (x-mean).powf(2.0)).sum();
                let stddev = stddev / (len - 1) as f32;
                let stddev = stddev.sqrt();

                let threshold = mean + 3.0*stddev;

                // Implements a basic outlier removal scheme that replaces
                // any point that is beyond 3*stddev of the mean with a
                // neighboring point

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

                signal = nuttall_window(signal);
                //signal = triangle_window(signal);

                signal.into_iter().chain(
                    std::iter::repeat(0.0f32)
                        .take(signal_max_len - len),
                )
            })
            .collect::<Vec<f32>>()[..];
        let stars = AF_Array::new(
            signals,
            // [ ] TODO 2nd term should be # of stars???
            AF_Dim4::new(&[signal_max_len as u64, num_stars as u64, 1, 1]),
        );


        // NOTE Remove DC constant of template to focus on signal
        //      - This is very important and will lead to false
        //        detection or searching for the wrong signal
        let stars_means = AF::mean(
            &stars,
            0,
        );

        let stars_means = AF::tile(
            &stars_means,
            AF_Dim4::new(
                &[signal_max_len as u64, 1, 1, 1]
            ),
        );

        let stars = AF::sub(&stars, &stars_means, false);

        /*
        let stars_max = AF::max(&stars, 0);
        let stars_min = AF::min(&stars, 0);
        let stars_min = AF::abs(&stars_min);

        let stars_stats = AF::join(0, &stars_max, &stars_min);
        let stars_max = AF::max(&stars_stats, 0);

        let stars_scales = AF::div(&1.0f32, &stars_max, false);
        let stars_scales = AF::tile(
            &stars_scales,
            AF_Dim4::new(
                &[signal_max_len as u64, 1, 1, 1]
            ),
        );

        let stars = AF::mul(&stars_scales, &stars, false);

        {
            let mut temp: Vec<f32> = Vec::new();

            let temp_stars = AF::col(&stars, 0);
            temp.resize(temp_stars.elements(), 0.0);

            temp_stars.lock();
            temp_stars.host(&mut temp);
            temp_stars.unlock();

            debug_plt(&temp[..], "blah", None);
        }
        */

        let stars = {
            let fft_bs = AF::fft(&stars, 1.0, templates[0].fft_len as i64);
            AF::rows(&fft_bs, 0, (templates[0].max_len - 1) as u64)
        };

        //stars.eval();

        //let stars = AF::conjg(&stars);
        //let stars = AF::transpose(&stars, false);
        // [ ] TODO work on making right grouping
        //     of templates output and max of them
        //     -- for now only works bc large template groups (only one group)
        for template_group in templates {
            // [ ] TODO add in Delta x scale
            let res_af = AF::matmul(
                &stars,
                &template_group.templates,
                AF::MatProp::TRANS,
                AF::MatProp::NONE,
            );

            //let stars = AF::transpose(&stars, false);
            /*
            let stars = AF::tile(&stars, AF_Dim4::new(
                &[num_stars as u64, template_group.num_templates as u64, 1, 1]));
            let res_af = AF::mul(
                &stars,
                &template_group.templates,
                false,
            );

            let res_af = AF::ifft(&res_af, 1.0, window_length as i64);
            */

            // as in SO questions try using abs to get pos. vals.
            // https://{{so}}.com/questions/6740545/understanding-fft-output
            // https://dsp.{{se}}.com/questions/20500/negative-values-of-the-fft
            // --- can be fixed will describe in other doc
            let res_af = AF::abs(&res_af);

            let res_af = AF::max(&res_af, 1);
            res_af.eval();

            let mut temp: Vec<f32> = Vec::new();
            temp.resize(res_af.elements(), 0.0);
            res_af.lock();
            res_af.host(&mut temp);
            res_af.unlock();

            res.append(&mut temp);
        }
    }

    res
}

pub fn uid_to_t0_tp(uid: &str) -> Option<(f32, f32)> {
    let adp_parser = Regex::new(
        r"(?x) # makes white space insignificant and adds comment support
                                  (\d+\.\d{3}) # sigma
                                  _(\d+\.\d{3}) # T_prime
                                  _(\d+\.\d{3}) # T
                                  _(\d+\.\d{3}) # A_prime
                                  _(\d+\.\d{3}) # relative
                                  _(\d+\.\d{3}).dat # phi",
    )
    .expect("Problem building adp_parser Regex.");

    adp_parser.captures(&uid).map(|caps| {
        let t_prime = caps
            .get(2)
            .expect("Problem getting t_prime match")
            .as_str()
            .parse::<f32>()
            .expect("Problem parsing t_prime match as f32");
        //println!("T': {}", t_prime);
        let samples_per_hour = 60.0 // minutes in an hour
            * (60.0/15.0); // samples in a second (240)
        let signal_time_in_samples = t_prime // length of signal (days)
            * 24.0 // hours in a day
            * samples_per_hour;
        let end_of_signal = 8.0 // hours per day
            * samples_per_hour
            * 24.0; // total sample days
        let center_of_signal = end_of_signal - (signal_time_in_samples / 2.0); // center of signal
        (center_of_signal, signal_time_in_samples)
    })
}

// [ ] TODO add _x_range functionality
#[allow(dead_code)]
pub fn debug_plt(data: &[f32], title: &str, _x_range: Option<&Vec<f32>>) {
    let c = inline_python::Context::new();

    //let (t0, _) = uid_to_t0_tp(title).unwrap();
    python! {
        #![context = &c]
        import matplotlib.pyplot as plt
        from unittest.mock import patch
        import sys
        sys.argv.append("test")

        plt.title('title)
        //plt.xticks([i for i in range(0, len('data), 2000)])
        plt.plot('data, marker="o", ls="")
        //plt.plot('t0, -1, marker="x", ls="")
        plt.show()
    }
}

#[allow(dead_code)]
pub fn debug_plt_2(data: &[f32], data2: &[f32], title: &str, skip_delta: u32) {
    let c = inline_python::Context::new();
    python! {
        #![context = &c]
        import matplotlib.pyplot as plt
        import numpy as np
        import sys
        sys.argv.append("test")

        temp = []
        for d in 'data:
            temp.append(d)
            for i in range(0, 'skip_delta-1):
                temp.append(None)

        temp2 = []
        // do this since we early terminate on successful detection
        // thus the lengths will be off
        //
        // will also be off by fragment logic but should be minor so ignore
        length = min(len('data2), len(temp))
        for i in range(0, length):
            if temp[i] is None:
                temp2.append(None)
            else:
                temp2.append(abs(temp[i]-'data2[i]))

        data2 = []
        skip = 16
        for i in range(0, len('data2), skip):
            data2.append('data2[i])
            for i in range(1, skip):
                data2.append(None)
        plt.title('title)
        plt.plot(temp, marker="o", ls="")
        plt.plot('data2, marker="x", ls="")
        //plt.plot(temp2, marker="s", ls="")
        plt.show()
    }
}

// since the each data path has a file located locally from it
// in the samples or arima_model_file, etc. we use this to get
// a proper localized/global path from our perspective and not the
// data config. file's perspective
//
// -- assumes file is a file on disk and not a directory due to
//    previous checks
pub fn normalize_local_data_paths(star_file: &str, data_file: &str) -> String {
    match Path::new(star_file).parent() {
        Some(base_dir) => [
            base_dir
                .to_str()
                .expect("Problem converting base_dir to string."),
            &data_file[..],
        ]
        .iter()
        .collect::<PathBuf>()
        .to_str()
        .expect("Problem converting normalized path to string.")
        .to_string(),
        None => data_file.to_string(),
    }
}
