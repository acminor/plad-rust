use inline_python::python;
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;

// NOTE: From NFD Paper TODO better cite
pub fn adp(t0: f32, t_prime: f32, sample_time: f32) -> f32 {
    ((sample_time - t0) / t_prime) * 100.0
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
pub fn debug_plt_2(
    data: &[f32],
    data2: &[f32],
    title: &str,
    skip_delta: u32,
    window_len: usize,
) {
    let c = inline_python::Context::new();
    python! {
        #![context = &c]
        import pickle
        import matplotlib.pyplot as plt
        import numpy as np
        import math
        import sys
        sys.argv.append("test")

        with open("temp.pickle", "wb+") as file:
            pickle.dump('data, file)

        temp = []
        // detector does not start output until
        // we have window length number of points
        // - shift to line up original data and results
        for d in  range(0, 'window_len):
            temp.append(None)
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
        //for i in range(0, len('data2), skip):
        //   data2.append('data2[i])
        //   for i in range(1, skip):
        //       data2.append(None)
        plt.title('title)
        plt.subplot(411)
        plt.plot(temp, marker="o", ls="")
        plt.subplot(412)
        plt.plot('data2, marker="x", ls="")
        plt.subplot(413)
        plt.plot(np.diff('data, n=1), marker="x", ls="")
        plt.subplot(414)
        plt.plot(np.diff('data, n=2), marker="x", ls="")
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
