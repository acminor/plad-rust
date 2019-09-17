use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;
use std::path::Path;
use std::path::PathBuf;
use inline_python::python;

use crate::template::*;

pub fn inner_product(
    templates: &[TemplateGroup],
    signals: &[Vec<f32>],
    window_length: usize,
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
    for signals in signals.chunks(signal_group_len) { //1) {//signal_group_len) {
        let num_stars = signals.len();
        let signals = &signals
            .iter()
            .flat_map(|signal| signal.iter())
            .cloned()
            .collect::<Vec<f32>>()[..];
        let stars = AF_Array::new(
            signals,
            // [ ] TODO 2nd term should be # of stars???
            AF_Dim4::new(&[window_length as u64,
                           num_stars as u64, 1, 1]),
        );

        let stars = {
            let fft_bs = AF::fft_r2c(&stars, 0.65, templates[0].fft_len as i64);
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

// [ ] TODO add _x_range functionality
pub fn debug_plt(data: &[f32], title: &str, _x_range: Option<&Vec<f32>>) {
    let c = inline_python::Context::new();
    python! {
        #![context = &c]
        import matplotlib.pyplot as plt
        from unittest.mock import patch
        import sys
        sys.argv.append("test")

        plt.title('title)
        plt.plot('data, marker="o", ls="")
        plt.show()
    }
}

pub fn debug_plt_2(data: &[f32], data2: &[f32], title: &str, window_len: usize) {
    let c = inline_python::Context::new();
    python! {
        #![context = &c]
        import matplotlib.pyplot as plt
        from unittest.mock import patch
        import sys
        sys.argv.append("test")

        temp = []
        for d in 'data:
            temp.append(d)
            for i in range(0, 'window_len - 1):
                temp.append(None)
        temp2 = []
        for i in range(0, len('data2)):
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
        Some(base_dir) => {
            [base_dir.to_str().unwrap(), &data_file[..]]
                .iter()
                .collect::<PathBuf>()
                .to_str().unwrap()
                .to_string()
        },
        None => {
            data_file.to_string()
        }
    }
}
