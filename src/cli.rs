use crate::dat_star;
use crate::json_star;
use crate::star::*;
use crate::sw_star::SWStar;
use crate::template::*;
use crate::toml_star;
use crate::gwac_reader::GWACReader;
use clap::{App, Arg};
use std::fs;
use std::str::FromStr;

pub struct RunInfo {
    pub templates: Templates,
    pub stars: Vec<SWStar>,
    pub gwac_reader: Option<GWACReader>,
    // [ ] TODO used for noise
    //  - should actually apply noise
    //    in generation of star data
    //    no need to add here
    pub detector_opts: DetectorOpts,
}

#[derive(Clone)]
pub struct DetectorOpts {
    pub _rho: f32,
    pub noise_stddev: f32,
    pub window_length: (usize, usize),
    pub fragment: u32,
    pub skip_delta: u32,
    pub alert_threshold: f32,
}

fn unwrap_parse_star_files(
    file: std::io::Result<fs::DirEntry>,
) -> Option<Star> {
    match file {
        Ok(file) => match file.file_type() {
            Ok(file_type) => {
                if file_type.is_file() {
                    match file.path().extension() {
                        Some(ext) if ext == "toml" => {
                            Some(toml_star::parse_star_file(
                                file.path().as_path().to_str().unwrap(),
                            ))
                        }
                        Some(ext) if ext == "dat" => {
                            Some(dat_star::parse_star_file(
                                file.path().as_path().to_str().unwrap(),
                            ))
                        }
                        Some(ext) if ext == "json" => {
                            json_star::parse_star_file(
                                file.path().as_path().to_str().unwrap(),
                            )
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
    }
}

fn parse_star_files(input_dirs: &[&str], detector_opts: &DetectorOpts) -> Vec<SWStar> {
    let input_dirs: Vec<String> = input_dirs
        .iter()
        .map(|s| s.to_string())
        .collect();
    // FIXME only doing one directory for now
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

    stars
        .into_iter()
        .zip((0..detector_opts.fragment).cycle())
        .map(|(star, fragment)| {
            SWStar::new()
                .set_star(star)
                .set_availables(fragment, detector_opts.skip_delta)
                .set_max_buffer_len(100)
                .set_window_lens(detector_opts.window_length.0 as u32,
                                 detector_opts.window_length.1 as u32)
                .build()
        })
        .collect::<Vec<SWStar>>()
}

pub fn parse_args() -> RunInfo {
    let matches = App::new("Matched Filter")
        .version(crate_version!())
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
                .required_unless("gwac_file")
                .conflicts_with("gwac_file")
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
                .conflicts_with_all(&["min_window_length", "max_window_length"])
                .required_unless_one(&["min_window_length", "max_window_length"]),
        )
        .arg(
            Arg::with_name("min_window_length")
                .long("min-window-length")
                .help("TODO")
                .requires("max_window_length")
                .required_unless("window_length")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("max_window_length")
                .long("max-window-length")
                .help("TODO")
                .requires("min_window_length")
                .required_unless("window_length")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("skip_delta")
                .long("skip-delta")
                .help("TODO")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("alert_threshold")
                .short("a")
                .long("alert-threshold")
                .help("TODO")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("fragment")
                .short("a")
                .long("fragment")
                .help("number of fragments to split stars into")
                .default_value("1")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("gwac_file")
                .long("gwacfile")
                .help("TODO")
                .takes_value(true)
                .required_unless("input_dir")
                .conflicts_with("input_dir")
        )
        .get_matches();

    let window_length = {
        match matches.value_of("window_length") {
            Some(win_len) => {
                let win_len = usize::from_str(win_len)
                    .expect("Trouble parsing window_length");
                (win_len, win_len)
            }
            None => {
                let min_len = matches
                    .value_of("min_window_len")
                    .expect("Must have window_length or min_window_len")
                    .parse::<usize>()
                    .expect("Trouble parsing min_window_len");
                let max_len = matches
                    .value_of("max_window_len")
                    .expect("Must have window_length or max_window_len")
                    .parse::<usize>()
                    .expect("Trouble parsing max_window_len");

                if max_len < min_len {
                    panic!(
                        "max_window_len must be greater than min_window_len"
                    );
                }

                (min_len, max_len)
            }
        }
    };

    let detector_opts = DetectorOpts {
        _rho: f32::from_str(matches.value_of("rho").unwrap()).unwrap(),
        noise_stddev: f32::from_str(matches.value_of("noise").unwrap())
            .unwrap(),
        window_length,
        skip_delta: matches
            .value_of("skip_delta")
            .unwrap()
            .parse::<u32>()
            .expect("Problem parsing skip_delta"),
        alert_threshold: f32::from_str(
            matches.value_of("alert_threshold").unwrap(),
        )
            .unwrap(),
        // TODO
        // - make plural
        // - add check for greater than 0
        fragment: matches
            .value_of("fragment")
            .unwrap()
            .parse::<u32>()
            .expect("Problem parsing fragment"),
    };

    let templates = parse_template_file(
        matches.value_of("templates_file").unwrap().to_string(),
    );

    // NOTE for simplicity do not allow offline and gwac_files
    //      to be on at same time
    if let Some(input_dirs) = matches.values_of("input_dir") {
        let stars = parse_star_files(&input_dirs.collect::<Vec<&str>>(), &detector_opts);

        return RunInfo {
            templates,
            stars,
            gwac_reader: None,
            // [ ] TODO see earlier fixme
            detector_opts,
        };
    }

    // NOTE for simplicity do not allow offline and gwac_files
    //      to be on at same time
    if let Some(gwac_file) = matches.value_of("gwac_file") {
        return RunInfo {
            templates,
            stars: Vec::new(),
            gwac_reader: Some(GWACReader::new(gwac_file)),
            // [ ] TODO see earlier fixme
            detector_opts,
        };
    }

    panic!("Should never make it here");
}
