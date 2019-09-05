use crate::dat_star;
use crate::star::*;
use crate::template::*;
use std::str::FromStr;
use clap::{App, Arg};
use std::fs;

pub struct RunInfo {
    pub templates: Templates,
    pub stars: Vec<Star>,
    // [ ] TODO used for noise
    //  - should actually apply noise
    //    in generation of star data
    //    no need to add here
    pub _rho: f32,
    pub noise_stddev: f32,
    pub window_length: i32,
}

fn unwrap_parse_star_files(file: std::io::Result<fs::DirEntry>) -> Option<Star> {
    match file {
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
    }
}

pub fn parse_args() -> RunInfo {
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

    let input_dirs: Vec<String> =
        matches.values_of("input_dir").unwrap().map(|s| s.to_string()).collect();
    let input_dir = &input_dirs[0];

    let stars: Vec<Star> = {
        match fs::metadata(&input_dir) {
            Ok(ref file_type) if file_type.is_dir() => fs::read_dir(&input_dir)
                .unwrap()
                .filter_map(unwrap_parse_star_files)
                .collect(),
            _ => panic!("Error in reading input_dir"),
        }
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
