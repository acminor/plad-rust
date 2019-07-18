//extern crate clap;

mod star;
mod template;
mod utils;

use star::*;
use template::*;
use utils::*;

use clap::{App, Arg};
use toml;

use std::boxed::Box;
use std::str::FromStr;
use std::vec;

use rustfft::num_complex::Complex;
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
            Arg::with_name("input_file")
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

    let star =
        parse_star_file(matches.value_of("input_file").unwrap().to_string());

    let stars = vec![star];

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
    let run_info = parse_args();

    let RunInfo {
        stars,
        templates,
        rho,
        noise_stddev,
        window_length,
    } = run_info;

    PROFILER.lock().unwrap().start("./prof.profile").expect("Couldn't start");

    let mut templates = templates;
    let template_sz = templates.templates.len();
    let mut stars = stars;

    let now = std::time::Instant::now();
    let mut iterations = 0;
    let iter = stars
        .into_iter()
        .zip(std::iter::repeat(templates.templates))
        .flat_map(|(mut star, templates)| {
            println!("Debug {}", templates.len());

            while star.samples.len() > (window_length as usize) {
                if iterations % 15 == 0 {
                    println!("Iteration: {}", iterations);
                }
                iterations += 1;

                let window =
                    star.samples.drain(0..(window_length as usize)).collect();

                templates
                    .iter()
                    .map(|template| {
                        inner_product(&template, &window, noise_stddev, true)
                            .iter()
                            .map(|x| x.re)
                            .collect::<Vec<f32>>()
                    })
                    .collect::<Vec<Vec<f32>>>();
            }

            vec![]
        })
        .collect::<Vec<Vec<f32>>>();

    PROFILER.lock().unwrap().stop().expect("Couldn't start");

    println!(
        "{} stars per second @ {} templates",
        iterations / now.elapsed().as_secs(),
        template_sz
    );

    println!("Hello, world!\n");
}
