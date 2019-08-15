//extern crate clap;

#[macro_use]
extern crate rental;

mod star;
mod template;
mod utils;
mod nnp;
mod python;

use star::*;
use template::*;
use utils::*;
use pyo3::prelude::*;

use arrayfire as AF;

use clap::{App, Arg};

use std::fs;
use std::str::FromStr;
use std::cell::RefCell;
use std::marker::PhantomData;

use cpuprofiler::PROFILER;

struct RunInfo {
    templates: Templates,
    stars: Vec<Star>,
    // [ ] TODO used for noise
    //  - should actually apply noise
    //    in generation of star data
    //    no need to add here
    _rho: f32,
    noise_stddev: f32,
    window_length: i32,
}

fn parse_args() -> RunInfo {
    let matches = App::new("Matched Filter")
        .version("0.1")
        .author("Austin C. Minor (米诺) <austin.chase.m@gmail.com>")
        .about("TODO")
        .arg(
            Arg::with_name("input_dir")
                .short("i")
                .long("input")
                .help("TODO")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("templates_file")
                .short("t")
                .long("templates_file")
                .help("TODO")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("rho")
                .short("p")
                .long("rho")
                .help("TODO")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("noise")
                .short("n")
                .long("noise")
                .help("TODO")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("window_length")
                .short("w")
                .long("window_length")
                .help("TODO")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let templates = parse_template_file(
        matches.value_of("templates_file").unwrap().to_string(),
    );

    let unwrap_parse_star_files =
        |file: std::io::Result<fs::DirEntry>| match file {
            Ok(file) => match file.file_type() {
                Ok(file_type) => {
                    if file_type.is_file() {
                        match file.path().extension() {
                            Some(ext) if ext == "toml" => {
                                Some(parse_star_file(
                                    file.path().as_path().to_str().unwrap(),
                                ))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                Err(_) => None,
            },
            Err(_) => None,
        };

    let input_dir = matches.value_of("input_dir").unwrap().to_string();
    let stars: Vec<Star> = match fs::metadata(&input_dir) {
        Ok(ref file_type) if file_type.is_dir() => fs::read_dir(&input_dir)
            .unwrap()
            .filter_map(unwrap_parse_star_files)
            .collect(),
        _ => panic!("Error in reading input_dir"),
    };

    println!("{}", stars.len());

    RunInfo {
        templates: templates,
        stars: stars,
        // [ ] TODO see earlier fixme
        _rho: f32::from_str(matches.value_of("rho").unwrap()).unwrap(),
        noise_stddev: f32::from_str(matches.value_of("noise").unwrap())
            .unwrap(),
        window_length: i32::from_str(
            matches.value_of("window_length").unwrap(),
        )
        .unwrap(),
    }
}

fn main() {
    {
        let mut gil_vec: Vec<GILGuard> = vec![];
        let mut py_vec: Vec<RefCell<Python>> = vec![];
        python_refs!(gil_vec, py_vec);
        python_refs!(gil_vec, py_vec);
        python_refs!(gil_vec, py_vec);
        let mut hm: std::collections::HashMap<String, String>
            = std::collections::HashMap::new();
        hm.insert("look_back".to_string(), "1".to_string());
        hm.insert("arima_model_file".to_string(), "1".to_string());
        let gil = Python::acquire_gil();
        let py = RefCell::new(gil.python());
        let n = nnp::NNPPredictor::new(py, hm);
        n.predict(vec![vec![3.0], vec![2.0], vec![1.0]], vec![0.0, 1.0, 2.0]);
    }
    let prof = false;
    let run_info = parse_args();

    AF::set_backend(AF::Backend::CUDA);

    let RunInfo {
        stars,
        templates,
        // [ ] TODO see earlier fixme
        _rho,
        noise_stddev,
        window_length,
    } = run_info;

    if prof {
        PROFILER
            .lock()
            .unwrap()
            .start("./prof.profile")
            .expect("Couldn't start");
    }

    let templates = templates;
    let mut stars = stars;

    let now = std::time::Instant::now();
    let mut iterations = 0;

    println!(
        "Total iterations needed: {}",
        ((stars[0].samples.len() as u64 / window_length as u64) as u64)
            * stars.len() as u64
    );

    let is_offline = true;
    let mut i = 0;
    let mut dbg_data: Vec<f32> = Vec::new();
    loop {
        let mut cur_stars = stars
            .iter_mut()
            .filter(|star| star.samples.len() >= (window_length as usize))
            .collect::<Vec<&mut Star>>();

        if cur_stars.len() == 0 && is_offline {
            break;
        }

        if i == -1 {
            break;
        } else {
            i+=1;
        }

        let windows = cur_stars
            .iter_mut()
            .map(|star| {
                if iterations % 15 == 0 {
                    println!("Iteration: {}", iterations);
                }
                iterations += 1;

                star.samples.drain(0..(window_length as usize)).collect()
            })
            .collect();

        let ip = inner_product(
            &templates.templates,
            &windows,
            noise_stddev,
            true,
            200,
            200,
        );

        dbg_data.push(ip[0]);
    }

    crate::utils::debug_plt(&dbg_data, None);

    if prof {
        PROFILER.lock().unwrap().stop().expect("Couldn't start");
    }

    println!(
        "{} stars per second @ {} templates",
        iterations / now.elapsed().as_secs(),
        1000
    );

    println!("Hello, world!\n");
}
