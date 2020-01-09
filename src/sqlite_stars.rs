use crate::star::{parse_model, Star, StarModelType, StarType};
use crate::utils;
use sqlite;
use std::{fs, io::Read};

#[derive(Debug, Deserialize)]
pub struct StarToml {
    pub id: String,
    pub star_type: String,
    pub samples: String,
    pub sample_rate: i32,
    pub arima_model_file: String,
}

pub fn parse_star_files(star_file: &str) -> Vec<Star> {
    let connection = sqlite::open(star_file).unwrap();

    let mut res = Vec::new();
    let mut statement = connection
        .prepare("SELECT * from StarEntry;")
        .unwrap();

    while let sqlite::State::Row = statement.next().unwrap() {
        let id = statement.read::<i64>(0).unwrap();
        let desc = statement.read::<String>(1).unwrap();
        let data = statement.read::<Vec<u8>>(2).unwrap();

        let star_toml: StarToml =
            toml::from_str(&desc).expect(&format!("Failed to parse Star TOML file: {}", star_file));

        let star_type = match star_toml.star_type.as_ref() {
            "constant" => StarType::Constant,
            "variable" => StarType::Variable,
            _ => StarType::Constant,
        };

        let samples = {
            let mut de = rmp_serde::Deserializer::new(&data[..]);

            serde::Deserialize::deserialize(&mut de)
                .expect("Failed to deserialize Star samples file")
        };

        res.push(
            Star {
                id: star_toml.id.clone(),
                uid: star_toml.id + "," + &star_file.to_string(),
                samples: Some(samples),
                samples_tick_index: std::cell::RefCell::new(0),
                star_type,
                model_type: StarModelType::None,
                model: parse_model(StarModelType::None, "".to_string()),
                sample_rate: star_toml.sample_rate,
            }
        )
    }

    res
}
