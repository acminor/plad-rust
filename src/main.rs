//extern crate clap;

mod star;
mod template;
mod utils;

use star::*;
use template::*;
use utils::*;

use clap::{App, Arg};

use std::fs;
use std::str::FromStr;

use cpuprofiler::PROFILER;

struct RunInfo {
    templates: Templates,
    stars: Vec<Star>,
    rho: f32,
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

    let unwrap_parse_star_files = |file: std::io::Result<fs::DirEntry>| match file {
        Ok(file) => match file.file_type() {
            Ok(file_type) => {
                if file_type.is_file() {
                    match file.path().extension() {
                        Some(ext) if ext == "toml" => Some(parse_star_file(
                            file.path().as_path().to_str().unwrap(),
                        )),
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

    //    let star =
    //        parse_star_file(matches.value_of("input_dir").unwrap().to_string());

    // let stars = vec![star];

    RunInfo {
        templates: templates,
        stars: stars,
        rho: f32::from_str(matches.value_of("rho").unwrap()).unwrap(),
        noise_stddev: f32::from_str(matches.value_of("noise").unwrap())
            .unwrap(),
        window_length: i32::from_str(
            matches.value_of("window_length").unwrap(),
        )
        .unwrap(),
    }
}

fn main() {
    let prof = false;
    let run_info = parse_args();

    let RunInfo {
        stars,
        templates,
        rho,
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
    let template_sz = templates.templates.len();
    let stars = stars;

    let now = std::time::Instant::now();
    let mut iterations = 0;
    let _iter = stars
        .into_iter()
        .flat_map(|mut star| {
            let mut res = Vec::new();
            while star.samples.len() > (window_length as usize) {
                if iterations % 15 == 0 {
                    println!("Iteration: {}", iterations);
                }
                iterations += 1;

                let window =
                    star.samples.drain(0..(window_length as usize)).collect();

                let max_filter_val = templates
                    .templates
                    .iter()
                    .map(|template| {
                        inner_product(&template, &window, noise_stddev, true)
                            .iter()
                            .map(|x| x.re)
                            .fold(-1000.0, |acc, x| match x > acc {
                                true => x,
                                false => acc,
                            })
                    })
                    .fold(-1000.0, |acc, x| match x > acc {
                        true => x,
                        false => acc,
                    });

                res.push(max_filter_val);
            }

            res
        })
        .collect::<Vec<f32>>();

    if prof {
        PROFILER.lock().unwrap().stop().expect("Couldn't start");
    }

    println!(
        "{} stars per second @ {} templates",
        iterations / now.elapsed().as_secs(),
        template_sz
    );

    println!("Hello, world!\n");
}
