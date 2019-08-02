use arrayfire as AF;
use arrayfire::device_mem_info;
use arrayfire::mem_info;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;
use gnuplot::{Color, Figure};
use num::Complex;
use rustfft::{
    num_complex::Complex as fft_complex, num_traits::Zero, FFTplanner,
};

use crate::template::*;

pub fn inner_product(
    templates: &Vec<TemplateGroup>,
    signals: &Vec<Vec<f32>>,
    snf: f32,
    pre_fft: bool,
    template_group_len: usize,
    signal_group_len: usize,
) -> Vec<f32> {
    let mut res: Vec<f32> = Vec::new();
    for signals in signals.chunks(signal_group_len) {
        let chunk_len = signals.len();
        let signals = &signals
            .iter()
            .flat_map(|signal| signal.into_iter())
            .cloned()
            .collect::<Vec<f32>>()[..];
        let stars = AF_Array::new(
            signals,
            AF_Dim4::new(&[30 as u64, (signals.len() / 30) as u64, 1, 1]),
        );

        let stars = AF::fft(&stars, 1.0, templates[0].max_len as i64);
        //let stars = AF::transpose(&stars, false);
        for template_group in templates {
            /*
            let padding = &AF::constant(
                Complex::new(0.0 as f32, 0.0 as f32),
                AF_Dim4::new(&[
                    chunk_len as u64,
                    (template_group.max_len - 30) as u64,
                    1,
                    1,
                ]),
            );
            println!("sd: {}", stars.dims());
            println!("pd: {}", padding.dims());
            let stars = AF::join(1, &stars, &padding);
            */
            //let stars = AF::fft(&stars, 1.0, template_group.max_len as i64);

            if false {
                let mut dbg_data: Vec<f32> = Vec::new();
                dbg_data.resize(stars.elements(), 0.0);
                stars.lock();
                AF::real(&stars).host(&mut dbg_data[..]);
                stars.unlock();

                let mut fg = Figure::new();
                fg.axes2d().lines(0..100000, dbg_data, &[Color("black")]);
                fg.show();
                std::thread::sleep_ms(100000);
            }

            println!("stars dim: {}", stars.dims());
            println!("temps dim: {}", template_group.templates.dims());
            let res_af = AF::matmul(
                &stars,
                &template_group.templates,
                AF::MatProp::TRANS,
                AF::MatProp::NONE,
            );

            let mut temp: Vec<f32> = Vec::new();
            temp.resize(res_af.elements(), 0.0);
            let res_af = AF::real(&res_af);
            res_af.lock();
            res_af.host(&mut temp);
            res_af.unlock();

            res.append(&mut temp);
        }
    }

    debug_plt(&res, None);

    res
}

pub fn debug_plt(data: &Vec<f32>, x_range: Option<&Vec<f32>>) {
    use std::process::Command;
    use plotters::prelude::*;
    use std::io::{self, Write};
    use tempfile::tempdir;
    use std::fs::File;

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

/*
if false {
let dbg_data: Vec<f32> = template.iter().map(|&x| x.re).collect();
let mut fg = Figure::new();
fg.axes2d()
.lines(0..template.len(), dbg_data, &[Color("black")]);
fg.show();
}
*/
