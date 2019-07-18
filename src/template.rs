use serde_derive::Deserialize;
use std::fs;
use std::io::Read;

use rustfft::{num_complex::Complex, num_traits::Zero, FFTplanner};

#[derive(Debug, Deserialize)]
pub struct TemplateToml {
    pub templates: String,
    pub pre_fft: bool,
}

#[derive(Debug)]
pub struct Templates {
    pub templates: Vec<Vec<Complex<f32>>>,
    pub pre_fft: bool,
}

pub fn parse_template_file(file_name: String) -> Templates {
    let contents = {
        let contents = fs::read_to_string(file_name)
            .expect("Failed to read Templates TOML file");

        contents
    };

    let template_toml: TemplateToml =
        toml::from_str(&contents).expect("Failed to parse Templates TOML file");

    let templates = {
        let mut file = fs::File::open(&template_toml.templates)
            .expect("Failed to read Templates templates file");
        let mut contents: Vec<u8> = Vec::new();
        file.read_to_end(&mut contents);

        let mut de = rmp_serde::Deserializer::new(&contents[..]);

        let temp: Vec<Vec<(f32, f32)>> =
            serde::Deserialize::deserialize(&mut de)
                .expect("Failed to deserialize templates");

        temp.into_iter()
            .map(|array| {
                array.iter().map(|&(x, y)| Complex::new(x, y)).collect()
            })
            .collect()
    };

    //let templates = Templates{
    //        pre_fft: template_toml.pre_fft
    //};

    Templates {
        templates: templates,
        pre_fft: true,
    }
}
