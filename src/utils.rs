use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;
use arrayfire::print_gen;
use std::path::Path;
use std::path::PathBuf;

use crate::template::*;

pub fn inner_product(
    templates: &Vec<TemplateGroup>,
    signals: &Vec<Vec<f32>>,
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
    for signals in signals.chunks(signal_group_len) {
        let _num_stars = signals.len();
        let signals = &signals
            .iter()
            .flat_map(|signal| signal.into_iter())
            .cloned()
            .collect::<Vec<f32>>()[..];
        let stars = AF_Array::new(
            signals,
            // [ ] TODO 2nd term should be # of stars???
            AF_Dim4::new(&[window_length as u64,
                           (signals.len() / window_length) as u64, 1, 1]),
        );

        let stars = {
            //let stars = AF::add(&stars, &(20.0 as f32), false);
            println!("{}", templates[0].fft_len);
            //let fft_bs = AF::fft_r2c(&stars, 1.0, templates[0].fft_len as i64);
            let fft_bs = AF::fft(&stars, 1.0, templates[0].fft_len as i64);
            AF::rows(&fft_bs, 0, (templates[0].max_len - 1) as u64)
        };

        stars.eval();

        //let stars = AF::conjg(&stars);
        //let stars = AF::transpose(&stars, false);
        // [ ] TODO work on making right grouping
        //     of templates output and max of them
        //     -- for now only works bc large template groups (only one group)
        for template_group in templates {
            //println!("stars dim: {}", stars.dims());
            //println!("temps dim: {}", template_group.templates.dims());

            // [ ] TODO add in Delta x scale
            let res_af = AF::matmul(
                &stars,
                &template_group.templates,
                AF::MatProp::TRANS,
                AF::MatProp::NONE,
            );

            let res_af = AF::transpose(&res_af, false);
            //println!("mult dims: {}", res_af.dims());

            //let res_af = AF::real(&res_af);
            // as in SO questions try using abs to get pos. vals.
            // https://{{so}}.com/questions/6740545/understanding-fft-output
            // https://dsp.{{se}}.com/questions/20500/negative-values-of-the-fft
            // --- can be fixed will describe in other doc
            let res_af = AF::abs(&res_af);

            let res_af = AF::max(&res_af, 0);
            res_af.eval();
            //println!("max dims: {}", res_af.dims());
            let mut temp: Vec<f32> = Vec::new();
            temp.resize(res_af.elements(), 0.0);
            res_af.lock();
            res_af.host(&mut temp);
            res_af.unlock();

            //debug_plt(&t2, None);
            res.append(&mut temp);
        }
    }

    res
}

// [ ] TODO add _x_range functionality
pub fn debug_plt(data: &Vec<f32>, _x_range: Option<&Vec<f32>>) {
    use std::process::Command;
    use plotters::prelude::*;
    use tempfile::tempdir;

    let mut max_val = -10000000.0;
    let mut min_val = 10000000.0;
    for &val in data {
        if val > max_val {
            max_val = val;
        }

        if val < min_val {
            min_val = val;
        }
    }

    let dir = tempdir().expect("trouble creating tmp dir");
    let img_path = dir.path().join("img.svg");
    {
        let root = SVGBackend::new(&img_path, (1280, 920))
            .into_drawing_area();
        root.fill(&White).expect("Trouble with plotting.");
        let mut chart = ChartBuilder::on(&root)
            .caption("Debug plot", ("Arial", 50).into_font())
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_ranged(0..data.len() as u64, min_val..max_val)
            .expect("Trouble building chart.");
        chart.configure_mesh().draw().expect("Trouble with plotting.");
        chart
            .draw_series(
                LineSeries::new(
                    data.iter().cloned().enumerate().map(|(x,y)| (x as u64, y)),
                    &Red
                )
            )
            .expect("Problem drawing data.");
    }

    Command::new("/usr/bin/eog")
        .arg(dir.path().join("img.svg"))
        .status()
        .expect("problem creating process");
}

// since the each data path has a file located locally from it
// in the samples or arima_model_file, etc. we use this to get
// a proper localized/global path from our perspective and not the
// data config. file's perspective
//
// -- assumes file is a file on disk and not a directory due to
//    previous checks
pub fn normalize_local_data_paths(star_file: &str, data_file: &String) -> String {
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
