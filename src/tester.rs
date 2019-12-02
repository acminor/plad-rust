use std::fs;

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
    pub fn new(desc_file: &str) -> TartanTester {
        let contents =
            fs::read_to_string(desc_file).expect("Failure to read Tartan Tester File");
        let desc =
            contents.parse::<toml::Value>().expect("Failure to parse Tartan Tester File");

        println!("{:?}", desc["signal"]["start_len"]);

        TartanTester {
            start_len: desc["signal"]["start_len"]
                .as_integer().expect("Problem parsing Tartan Tester File: start_len") as usize,
            end_len: desc["signal"]["end_len"]
                .as_integer().expect("Problem parsing Tartan Tester File: end_len") as usize,
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
