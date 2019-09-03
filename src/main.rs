//extern crate clap;

#[macro_use]
extern crate rental;

#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;

extern crate jemallocator;

// [ ] TODO Test if this speeds up the program: also what about memory pressure
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod star;
mod template;
mod utils;
mod python;
mod dat_star;

use star::*;
use template::*;
use utils::*;

use arrayfire as AF;

use clap::{App, Arg};

use std::fs;
use std::str::FromStr;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::collections::HashMap;

use slog::Drain;

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
                .number_of_values(1)
                .multiple(true)
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("templates_file")
                .short("t")
                .long("templates-file")
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
                .long("window-length")
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
                            },
                            Some(ext) if ext == "dat" => {
                                Some(dat_star::parse_star_file(
                                    file.path().as_path().to_str().unwrap(),
                                ))
                            },
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

    let input_dirs: Vec<String> =
        matches.values_of("input_dir").unwrap().map(|s| s.to_string()).collect();
    let input_dir = &input_dirs[0];
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

fn setup_logging() -> slog::Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    slog::Logger::root(drain, o!())
}

fn main() {
    {
        let mut hm: std::collections::HashMap<String, String>
            = std::collections::HashMap::new();
        hm.insert("look_back".to_string(), "1".to_string());
        hm.insert("arima_model_file".to_string(), "1".to_string());
    }

    AF::info();

    let prof = false;
    let log = setup_logging();
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
    let mut log_timer = std::time::Instant::now();
    /* NOTE: This is a per star, per window counting variable */
    let mut iterations = 0;

    let max_len: usize
        = stars.iter().map(|star| star.samples.len()).max().unwrap();
    let tot_iter: usize =
        stars.iter().map(|star| star.samples.len()).sum::<usize>()
        / window_length as usize;

    println!(
        "Total iterations needed: {}",
        tot_iter
    );

    let is_offline = true;
    let mut i = 0;
    let mut dbg_data: Vec<f32> = Vec::new();
    let mut data: HashMap<String, Vec<f32>> = HashMap::new();
    stars.iter()
        .for_each(|star| {
            data.insert(star.uid.clone(), Vec::new());
        });
    loop {
        if log_timer.elapsed() > std::time::Duration::from_secs(2) {
            // TODO implement logging logic
            let sps = iterations as f32 / now.elapsed().as_secs() as f32;
            let pp = (iterations as f32)/(tot_iter as f32) * 100.0;
            info!(log, "";
                  "TotTime"=>format!("{}s", now.elapsed().as_secs()),
                  "IterationsLeft"=>format!("{}", tot_iter - iterations as usize),
                  "EstTimeLeft"=>format!("{}s", (tot_iter - iterations as usize)
                                         as f32/sps as f32),
                  "StarsPerSec"=>format!("{}", sps),
                  "StarsPerTenSec"=>format!("{}", sps*10.0),
                  "%Progress"=>format!("{}%", pp));

            log_timer = std::time::Instant::now();
        }
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
                iterations += 1;

                star.samples.drain(0..(window_length as usize)).collect()
            })
            .collect();

        let window_names = cur_stars
            .iter()
            .map(|star| {
                &star.uid[..]
            })
            .collect::<Vec<&str>>();

        let ip = inner_product(
            &templates.templates,
            &windows,
            noise_stddev,
            true,
            200,
            200,
        );

        ip
            .iter()
            .zip(window_names)
            .for_each(|(val, star)| {
                data.get_mut(star).unwrap().push(*val);
            });

        dbg_data.push(ip[0]);
    }

    let stats = |data: &Vec<f32>| {
        let mut avg = 0.0;
        let mut min = std::f32::INFINITY;
        let mut max = std::f32::NEG_INFINITY;
        let mut std_dev = 0.0;
        let len = data.len() as f32;

        for &datum in data {
            avg += datum;
            min = if min < datum {
                min
            } else {
                datum
            };
            max = if max > datum {
                max
            } else {
                datum
            };
            std_dev += datum*datum;
        }

        avg = avg/len;
        std_dev = (std_dev/len - avg*avg).sqrt();

        (min, max, avg, std_dev)
    };

    { // over all values
        let (min, max, avg, std_dev) =
            stats(&data
                  .iter()
                  .flat_map(|(key, val)| {
                      val.clone()
                  }).collect());
        info!(log, "All values stats: ";
              "min"=>format!("{}", min),
              "max"=>format!("{}", max),
              "avg"=>format!("{}", avg),
              "std_dev"=>format!("{}", std_dev));
    }

    { // TODO comment ???
        let star_stats = || {
            data
                .iter()
                .map(|(key, val)| {
                    stats(val)
                })
        };

        let mins = star_stats()
            .by_ref()
            .map(|tup| {tup.0})
            .collect::<Vec<f32>>();
        let maxs = star_stats()
            .by_ref()
            .map(|tup| {tup.1})
            .collect::<Vec<f32>>();
        let avgs = star_stats()
            .by_ref()
            .map(|tup| {tup.2})
            .collect::<Vec<f32>>();

        let (min, max, avg, std_dev) = stats(&mins);
        info!(log, "Min values stats: ";
              "min"=>format!("{}", min),
              "max"=>format!("{}", max),
              "avg"=>format!("{}", avg),
              "std_dev"=>format!("{}", std_dev));

        let (min, max, avg, std_dev) = stats(&maxs);
        info!(log, "Max values stats: ";
              "min"=>format!("{}", min),
              "max"=>format!("{}", max),
              "avg"=>format!("{}", avg),
              "std_dev"=>format!("{}", std_dev));

        let (min, max, avg, std_dev) = stats(&avgs);
        info!(log, "Avg values stats: ";
              "min"=>format!("{}", min),
              "max"=>format!("{}", max),
              "avg"=>format!("{}", avg),
              "std_dev"=>format!("{}", std_dev));

        /*
        info!(log, "All values stats: ";
              "min"=>format!("{}", min),
              "max"=>format!("{}", max),
              "avg"=>format!("{}", avg),
              "std_dev"=>format!("{}", std_dev));
        */
    }

    crate::utils::debug_plt(&dbg_data, None);

    if prof {
        PROFILER.lock().unwrap().stop().expect("Couldn't start");
    }

    /*
    println!(
        "{} stars per second @ {} templates",
        iterations / now.elapsed().as_secs(),
        1000
    );
    */

    println!("Hello, world!\n");
}
