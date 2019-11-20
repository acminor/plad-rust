use crate::star::{parse_model, Star, StarModelType, StarType};
use crate::utils;
use std::{fs, io::Read};

#[derive(Debug, Deserialize)]
pub struct StarToml {
    pub id: String,
    pub star_type: String,
    pub samples: String,
    pub sample_rate: i32,
    pub arima_model_file: String,
}

pub fn parse_star_file(star_file: &str) -> Star {
    let contents =
        fs::read_to_string(&star_file).expect("Failed to read Star TOML file");
    let star_toml: StarToml =
        toml::from_str(&contents).expect("Failed to parse Star TOML file");

    let star_type = match star_toml.star_type.as_ref() {
        "constant" => StarType::Constant,
        "variable" => StarType::Variable,
        _ => StarType::Constant,
    };

    let samples = {
        let mut file = fs::File::open(&utils::normalize_local_data_paths(
            &star_file,
            &star_toml.samples,
        ))
        .expect("Failed to read Star samples file");
        let mut contents: Vec<u8> = Vec::new();
        file.read_to_end(&mut contents)
            .expect("Failed reading contents of Star samples.");

        let mut de = rmp_serde::Deserializer::new(&contents[..]);

        serde::Deserialize::deserialize(&mut de)
            .expect("Failed to deserialize Star samples file")
    };

    Star {
        id: star_toml.id.clone(),
        uid: star_toml.id + &star_file.to_string(),
        samples: Some(samples),
        samples_tick_index: std::cell::RefCell::new(0),
        star_type,
        model_type: StarModelType::None,
        model: parse_model(StarModelType::None, "".to_string()),
        sample_rate: star_toml.sample_rate,
    }
}
