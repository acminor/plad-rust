use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;
use inline_python::python;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;

use crate::template::*;

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
        let signal_max_len =
            signals.iter().map(|signal| signal.len()).max().unwrap();
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
            .flat_map(|signal| {
                signal.iter().chain(
                    std::iter::repeat(&0.0f32)
                        .take(signal_max_len - signal.len()),
                )
            })
            .cloned()
            .collect::<Vec<f32>>()[..];
        let stars = AF_Array::new(
            signals,
            // [ ] TODO 2nd term should be # of stars???
            AF_Dim4::new(&[signal_max_len as u64, num_stars as u64, 1, 1]),
        );

        let stars = {
            let fft_bs = AF::fft_r2c(&stars, 1.0, templates[0].fft_len as i64);
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
    .unwrap();

    adp_parser.captures(&uid).map(|caps| {
        let t_prime = caps.get(2).unwrap().as_str().parse::<f32>().unwrap();
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
        plt.title('title)
        plt.plot(temp, marker="o", ls="")
        plt.plot('data2, marker="x", ls="")
        plt.plot(temp2, marker="s", ls="")
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
        Some(base_dir) => [base_dir.to_str().unwrap(), &data_file[..]]
            .iter()
            .collect::<PathBuf>()
            .to_str()
            .unwrap()
            .to_string(),
        None => data_file.to_string(),
    }
}
