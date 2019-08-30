use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;
use num::Complex;
use std::path::Path;
use std::path::PathBuf;

use crate::template::*;

pub fn inner_product(
    templates: &Vec<TemplateGroup>,
    signals: &Vec<Vec<f32>>,
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
        let num_stars = signals.len();
        let signals = &signals
            .iter()
            .flat_map(|signal| signal.into_iter())
            .cloned()
            .collect::<Vec<f32>>()[..];
        let stars = AF_Array::new(
            signals,
            AF_Dim4::new(&[30 as u64, (signals.len() / 30) as u64, 1, 1]),
        );

        let stars = {
            let fft_bs = AF::fft(&stars, 1.0, templates[0].fft_len as i64);
            let mut buf: Vec<Complex<f32>> = Vec::new();
            buf.resize(fft_bs.elements(),
                       Complex::new(0.0 as f32,0.0 as f32));

            fft_bs.lock();
            fft_bs.host(&mut buf);
            fft_bs.unlock();

            let mut fft: Vec<Vec<Complex<f32>>> = Vec::new();
            for _ in 0..num_stars {
                let mut temp = Vec::new();
                temp.append(&mut buf
                            .drain(0..templates[0].max_len)
                            .collect::<Vec<Complex<f32>>>()
                );
                fft.push(temp);
            }

            let fft = fft
                .into_iter()
                .flat_map(|star_fft|
                          star_fft.into_iter()).collect::<Vec<Complex<f32>>>();

            AF_Array::new(
                &fft,
                AF_Dim4::new(&[
                    templates[0].max_len as u64, num_stars as u64, 1, 1])
            )
        };
        //let stars = AF::transpose(&stars, false);
        for template_group in templates {
            println!("stars dim: {}", stars.dims());
            println!("temps dim: {}", template_group.templates.dims());

            // [ ] TODO add in Delta x scale
            let res_af = AF::matmul(
                &stars,
                &template_group.templates,
                AF::MatProp::TRANS,
                AF::MatProp::NONE,
            );

            let res_af = AF::transpose(&res_af, false);
            println!("mult dims: {}", res_af.dims());

            let mut temp: Vec<f32> = Vec::new();
            temp.resize(res_af.elements(), 0.0);
            let res_af = AF::real(&res_af);
            res_af.lock();
            res_af.host(&mut temp);
            res_af.unlock();

            let mut t2 = temp.chunks(template_group.num_templates).map(|mf_outs| {
                let mut max = -1000.0;
                for &out in mf_outs {
                    if out < 0.0 {
                        println!("WARNING: template with less than 0 value");
                    }

                    max = if out/0.0006 > max {
                        out/0.0006
                    } else {
                        max
                    };
                }

                max
            }).collect::<Vec<f32>>();

            //debug_plt(&t2, None);
            res.append(&mut t2);
        }
    }

    if false {
        debug_plt(&res, None);
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
