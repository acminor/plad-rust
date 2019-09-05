use serde_derive::Deserialize;
use std::{fs, io::Read};
use crate::utils;

#[derive(Debug, Deserialize)]
pub struct StarToml {
    pub id: String,
    pub star_type: String,
    pub samples: String,
    pub sample_rate: i32,
    pub arima_model_file: String,
}

#[derive(Debug)]
pub enum StarType {
    Constant,
    Variable,
    Unknown,
}

#[derive(Debug)]
pub enum StarModelType {
    //Lstm,
    //Arima,
    None,
}

pub struct Star {
    pub id: String,
    pub uid: String,
    pub samples: Vec<f32>,
    pub star_type: StarType,
    pub model_type: StarModelType,
    pub model: Box<dyn StarModel + Send>,
    pub sample_rate: i32,
}

pub struct StarModelInitErrMsg {
    _problem_entry: String,
    _err_msg: String,
}

pub type StarModelErr = Result<(), StarModelInitErrMsg>;
pub trait StarModel {
    fn init(&self, args: std::collections::HashMap<String, String>)
            -> StarModelErr;
    fn predict(&self, look_backs: Vec<Vec<f32>>, times: Vec<f32>) -> f32;
}

#[derive(Debug)]
pub struct NoneModel();

impl StarModel for NoneModel {
    fn init(&self, _args: std::collections::HashMap<String, String>)
            -> StarModelErr
    {
        Ok(())
    }
    fn predict(&self, _look_backs: Vec<Vec<f32>>, _times: Vec<f32>) -> f32 {
        0.0
    }
}

// [ ] TODO implement model functionality
pub fn parse_model(mtype: StarModelType, _mfile: String)
                   -> Box<dyn StarModel + Send> {
    match mtype {
        StarModelType::None => Box::new(NoneModel {}),
        //_ => Box::new(NoneModel {}),
    }
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
        let mut file = fs::File::open(
            &utils::normalize_local_data_paths(&star_file, &star_toml.samples)
            ).expect("Failed to read Star samples file");
        let mut contents: Vec<u8> = Vec::new();
        file.read_to_end(&mut contents)
            .expect("Failed reading contents of Star samples.");

        let mut de = rmp_serde::Deserializer::new(&contents[..]);

        serde::Deserialize::deserialize(&mut de)
            .expect("Failed to deserialize Star samples file")
    };

    Star {
        id: star_toml.id,
        uid: star_file.to_string(),
        samples: samples,
        star_type: star_type,
        model_type: StarModelType::None,
        model: parse_model(StarModelType::None, "".to_string()),
        sample_rate: star_toml.sample_rate,
    }
}
