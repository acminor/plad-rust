use std::collections::HashMap;
use std::fs;

pub trait Tester {
    fn is_true_positive(&self, star: &str, sample_time: usize) -> bool;
    fn is_false_positive(&self, star: &str, sample_time: usize) -> bool {
        !self.is_true_positive(star, sample_time)
    }

    fn is_valid(&self) -> bool {
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
    fn is_true_positive(&self, _star: &str, _sample_time: usize) -> bool {
        true
    }

    fn _adp(&self, _star: &str, _sample_time: usize) -> f32 {
        0.0
    }
}

pub struct TartanTester {
    #[allow(unused)]
    start_len: usize,
    #[allow(unused)]
    end_len: usize,
}

impl TartanTester {
    pub fn new(desc_file: &str) -> TartanTester {
        let contents = fs::read_to_string(desc_file)
            .expect("Failure to read Tartan Tester File");
        let desc = contents
            .parse::<toml::Value>()
            .expect("Failure to parse Tartan Tester File");

        println!("{:?}", desc["signal"]["start_len"]);

        TartanTester {
            start_len: desc["signal"]["start_len"]
                .as_integer()
                .expect("Problem parsing Tartan Tester File: start_len")
                as usize,
            end_len: desc["signal"]["end_len"]
                .as_integer()
                .expect("Problem parsing Tartan Tester File: end_len")
                as usize,
        }
    }

    #[allow(unused)]
    fn star_name_to_len(star: &str) -> usize {
        star.split(",")
            .filter(|kv| kv.contains("len"))
            .map(|kv| kv.split("=").collect::<Vec<&str>>())
            .collect::<Vec<Vec<&str>>>()[0][1] // only one entry and 1 is value, 0 is key
            .parse::<usize>()
            .expect("malformed tartan star name")
    }

    fn star_name_to_attrs(star: &str) -> HashMap<String, String> {
        star.split(",")
            .map(|kv| {
                let temp = kv
                    .split("=")
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>();

                (temp[0].clone(), temp[1].clone())
            })
            .collect::<HashMap<String, String>>()
    }
}

impl Tester for TartanTester {
    // TODO XXX: check what units sample_time is in
    fn is_true_positive(&self, star: &str, sample_time: usize) -> bool {
        let attrs = TartanTester::star_name_to_attrs(star);

        let t_left = attrs["tl"].parse::<usize>().unwrap();
        let t_right = attrs["tr"].parse::<usize>().unwrap();

        // TODO should be equals or just strict inequality???
        // - shouldn't matter much b/c we shouldn't be able to
        //   predict immediately anyway
        sample_time > t_left && sample_time < t_right
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn _adp(&self, star: &str, sample_time: usize) -> f32 {
        let attrs = TartanTester::star_name_to_attrs(star);

        let t_left = attrs["tl"].parse::<usize>().unwrap();
        let t_right = attrs["tr"].parse::<usize>().unwrap();
        // center of signal
        let t_peak = (t_left + t_right) as f32 / 2.0;
        let signal_width = t_right - t_left;

        crate::utils::adp(t_peak, signal_width as f32, sample_time as f32)
    }
}

pub struct NFDTester {}

impl Tester for NFDTester {
    fn is_true_positive(&self, _star: &str, sample_time: usize) -> bool {
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
