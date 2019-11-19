use crate::dat_star;
use crate::gwac_reader::GWACReader;
use crate::json_star;
use crate::star::*;
use crate::sw_star::SWStar;
use crate::template::*;
use crate::toml_star;
use crate::filter_utils::WindowFunc;
use clap::{App, Arg};
use std::fs;
use std::str::FromStr;
use toml;

pub struct RunInfo {
    pub templates: Templates,
    pub stars: Vec<SWStar>,
    pub gwac_reader: Option<GWACReader>,
    // [ ] TODO used for noise
    //  - should actually apply noise
    //    in generation of star data
    //    no need to add here
    pub detector_opts: DetectorOpts,
    pub log_opts: LogOpts,
    pub tester: Box<dyn Tester>,
}

arg_enum!{
    pub enum SortOpt {
        None,
        Increasing,
        Decreasing,
    }
}

pub trait Tester {
    fn is_true_positive(&self, star: &str, sample_time: usize) -> bool;
    fn is_false_positive(&self, star: &str, sample_time: usize) -> bool {
        !self.is_true_positive(star, sample_time)
    }

    fn is_valid(&self) -> bool{
        false
    }

    fn _adp(&self, star: &str, sample_time: usize) -> f32;
    fn adp(&self, star: &str, sample_time: usize) -> f32 {
        if self.is_false_positive(star, sample_time) {
            panic!("ADP called on an invalid value")
        }

        self._adp(star, sample_time)
    }
}

pub struct NoneTester {}

impl Tester for NoneTester {
    fn is_true_positive(&self, star: &str, sample_time: usize) -> bool {
        true
    }
    fn _adp(&self, star: &str, sample_time: usize) -> f32 {
        0.0
    }
}

pub struct TartanTester {
    start_len: usize,
    end_len: usize,
}

impl TartanTester {
    fn new(desc_file: &str) -> TartanTester {
        let contents =
            fs::read_to_string(desc_file).expect("Failure to read Tartan Tester File");
        let desc =
            contents.parse::<toml::Value>().expect("Failure to parse Tartan Tester File");

        TartanTester {
            start_len: desc["signal"]["start_len"]
                .as_str().expect("Problem parsing Tartan Tester File")
                .parse::<usize>().expect("malformed tartan tester file"),
            end_len: desc["signal"]["end_len"]
                .as_str().expect("Problem parsing Tartan Tester File")
                .parse::<usize>().expect("malformed tartan tester file"),
        }
    }
    fn star_name_to_len(star: &str) -> usize {
        star
            .split(",")
            .filter(|kv| kv.contains("len"))
            .map(|kv| kv.split("=").collect::<Vec<&str>>())
            .collect::<Vec<Vec<&str>>>()[0][1] // only one entry and 1 is value, 0 is key
            .parse::<usize>().expect("malformed tartan star name")
    }
}

impl Tester for TartanTester {
    fn is_true_positive(&self, star: &str, sample_time: usize) -> bool {
        let tot_len = TartanTester::star_name_to_len(star);

        // between the start and end boundaries
        // -- FIXME should be equality???
        // ---- Shouldn't mater much (b/c shouldn't predict immediately)
        sample_time > self.start_len && sample_time < (tot_len - self.end_len)
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn _adp(&self, star: &str, sample_time: usize) -> f32 {
        let tot_len = TartanTester::star_name_to_len(star);
        let signal_width = (tot_len - (self.start_len + self.end_len)) as f32;
        // NOTE: ignores discrete values and approximates as continuous
        let center_of_signal = signal_width/2.0 + self.start_len as f32;

        crate::utils::adp(center_of_signal, signal_width, sample_time as f32)
    }
}

pub struct NFDTester {
}

impl Tester for NFDTester {
    fn is_true_positive(&self, star: &str, sample_time: usize) -> bool {
        // FIXME we can make this more accurate XD
        sample_time >= 40320 && sample_time <= 46080
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn _adp(&self, star: &str, sample_time: usize) -> f32 {
        if let Some((t0, t_prime)) = crate::utils::uid_to_t0_tp(star) {
            crate::utils::adp(t0, t_prime, sample_time as f32)
        } else {
            panic!("Issue parsing t0, t_prime from NFD star")
        }
    }
}

pub struct TartanTestData {
}

pub enum TestType {
    Tartan(TartanTestData),
    None,
}

pub struct LogOpts {
    pub sort: SortOpt,
    pub plot: bool,
}

#[derive(Clone)]
pub struct DetectorOpts {
    pub _rho: f32,
    pub noise_stddev: f32,
    pub window_length: (usize, usize),
    pub fragment: u32,
    pub skip_delta: u32,
    pub alert_threshold: f32,
    pub window_func: WindowFunc,
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
                                file.path().as_path().to_str().expect("Problem converting toml file name to string"),
                            ))
                        }
                        Some(ext) if ext == "dat" => {
                            Some(dat_star::parse_star_file(
                                file.path().as_path().to_str().expect("Problem converting dat file name to string"),
                            ))
                        }
                        Some(ext) if ext == "json" => {
                            json_star::parse_star_file(
                                file.path().as_path().to_str().expect("Problem converting json file name to string"),
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

fn parse_star_files(
    input_dirs: &[&str],
    detector_opts: &DetectorOpts,
) -> Vec<SWStar> {
    let input_dirs: Vec<String> =
        input_dirs.iter().map(|s| s.to_string()).collect();
    // FIXME only doing one directory for now
    let input_dir = &input_dirs[0];

    let stars: Vec<Star> = {
        match fs::metadata(&input_dir) {
            Ok(ref file_type) if file_type.is_dir() => fs::read_dir(&input_dir)
                .expect("Problem reading star input directory.")
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
                .set_window_lens(
                    detector_opts.window_length.0 as u32,
                    detector_opts.window_length.1 as u32,
                )
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
                .required_unless_one(&["gwac_file", "license"])
                .conflicts_with_all(&["gwac_file", "license"]),
        )
        .arg(
            Arg::with_name("templates_file")
                .short("t")
                .long("templates-file")
                .help("TODO")
                .takes_value(true)
                .conflicts_with("license")
                .required_unless("license")
                .required(true),
        )
        .arg(
            Arg::with_name("rho")
                .short("p")
                .long("rho")
                .help("TODO")
                .takes_value(true)
                .conflicts_with("license")
                .required_unless("license")
                .required(true),
        )
        .arg(
            Arg::with_name("noise")
                .short("n")
                .long("noise")
                .help("TODO")
                .takes_value(true)
                .conflicts_with("license")
                .required_unless("license")
                .required(true),
        )
        .arg(
            Arg::with_name("window_length")
                .short("w")
                .long("window-length")
                .help("TODO")
                .takes_value(true)
                .conflicts_with_all(&[
                    "min_window_length",
                    "max_window_length",
                    "license",
                ])
                .required_unless_one(&[
                    "min_window_length",
                    "max_window_length",
                    "license",
                ]),
        )
        .arg(
            Arg::with_name("min_window_length")
                .long("min-window-length")
                .help("TODO")
                .requires("max_window_length")
                .required_unless_one(&["window_length", "license"])
                .conflicts_with("license")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("max_window_length")
                .long("max-window-length")
                .help("TODO")
                .requires("min_window_length")
                .required_unless_one(&["window_length", "license"])
                .conflicts_with("license")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("skip_delta")
                .long("skip-delta")
                .help("TODO")
                .takes_value(true)
                .conflicts_with("license")
                .required_unless("license")
                .required(true),
        )
        .arg(
            Arg::with_name("alert_threshold")
                .short("a")
                .long("alert-threshold")
                .help("TODO")
                .takes_value(true)
                .conflicts_with("license")
                .required_unless("license")
                .required(true),
        )
        .arg(
            Arg::with_name("fragment")
                .long("fragment")
                .help("number of fragments to split stars into")
                .takes_value(true)
                .conflicts_with("license")
                .required_unless("license")
                .required(true),
        )
        .arg(
            Arg::with_name("gwac_file")
                .long("gwac-file")
                .help("TODO")
                .takes_value(true)
                .required_unless_one(&["input_dir", "license"])
                .conflicts_with_all(&["input_dir", "license"]),
        )
        .arg(
            Arg::with_name("window_function")
                .long("window-func")
                .help("TODO")
                .takes_value(true)
                .default_value("triangle")
                .possible_values(&WindowFunc::variants())
                .case_insensitive(true)
        )
        .arg(
            Arg::with_name("sort")
                .long("sort")
                .help("TODO")
                .takes_value(true)
                .default_value("none")
                .possible_values(&SortOpt::variants())
                .case_insensitive(true)
        )
        .arg(
            Arg::with_name("plot")
                .long("plot")
                .help("TODO")
                .takes_value(true)
                .default_value("true")
                .possible_values(&["true", "false"])
                .case_insensitive(true)
        )
        .arg(
            Arg::with_name("tartan_test")
                .long("tartan-test")
                .help("TODO")
                .takes_value(true)
                .default_value("false")
                .possible_values(&["true", "false"])
                .case_insensitive(true)
        )
        .arg(
            Arg::with_name("tartan_test_file")
                .long("tartan-test-file")
                .help("TODO")
                .takes_value(true)
                .required_if("tartan_test", "true")
        )
        .arg(
            Arg::with_name("license")
                .long("license")
                .help("Display license and attribution information."),
        )
        .get_matches();

    println!("{}\n\n", include_str!("../COPYRIGHT"));

    if matches.is_present("license") {
        println!(
            "{}\n\n{}",
            include_str!("../LICENSE"),
            include_str!("../CREDITS")
        );
        std::process::exit(0);
    }

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
        _rho: f32::from_str(
            matches.value_of("rho").expect("Problem reading rho."),
        )
        .expect("Problem parsing rho."),
        noise_stddev: f32::from_str(
            matches.value_of("noise").expect("Problem reading noise"),
        )
        .expect("Problem parsing noise"),
        window_length,
        skip_delta: matches
            .value_of("skip_delta")
            .expect("Problem reading skip_delta")
            .parse::<u32>()
            .expect("Problem parsing skip_delta"),
        alert_threshold: f32::from_str(
            matches
                .value_of("alert_threshold")
                .expect("Problem reading alert_threshold"),
        )
        .expect("Problem parsing alert_threshold"),
        // TODO
        // - make plural
        // - add check for greater than 0
        fragment: matches
            .value_of("fragment")
            .expect("Problem reading fragment")
            .parse::<u32>()
            .expect("Problem parsing fragment"),
        window_func: value_t_or_exit!(matches, "window_function", WindowFunc),
    };

    let log_opts = LogOpts {
        sort: value_t_or_exit!(matches, "sort", SortOpt),
        plot: value_t_or_exit!(matches, "plot", bool),
    };

    let test_type = if value_t!(matches, "tartan_test", bool).is_ok() {
        TestType::Tartan(TartanTestData{})
    } else {
        TestType::None
    };

    let templates = parse_template_file(
        matches
            .value_of("templates_file")
            .expect("Problem reading templates_file")
            .to_string(),
    );

    let tester = Box::new(NFDTester{});

    // NOTE for simplicity do not allow offline and gwac_files
    //      to be on at same time
    if let Some(input_dirs) = matches.values_of("input_dir") {
        let stars = parse_star_files(
            &input_dirs.collect::<Vec<&str>>(),
            &detector_opts,
        );

        return RunInfo {
            templates,
            stars,
            gwac_reader: None,
            // [ ] TODO see earlier fixme
            detector_opts,
            log_opts,
            tester,
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
            log_opts,
            tester,
        };
    }

    panic!("Should never make it here");
}
