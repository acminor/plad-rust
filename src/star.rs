use std::cell::RefCell;

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
    pub star_type: StarType,
    pub model_type: StarModelType,
    pub model: Box<dyn StarModel + Send>,
    pub sample_rate: i32,
    // Used to run on offline data
    pub samples: Option<Vec<f32>>,
    pub samples_tick_index: RefCell<usize>,
}

pub struct StarModelInitErrMsg {
    _problem_entry: String,
    _err_msg: String,
}

pub type StarModelErr = Result<(), StarModelInitErrMsg>;
pub trait StarModel {
    fn init(
        &self,
        args: std::collections::HashMap<String, String>,
    ) -> StarModelErr;
    fn predict(&self, look_backs: Vec<Vec<f32>>, times: Vec<f32>) -> f32;
}

#[derive(Debug)]
pub struct NoneModel();

impl StarModel for NoneModel {
    fn init(
        &self,
        _args: std::collections::HashMap<String, String>,
    ) -> StarModelErr {
        Ok(())
    }
    fn predict(&self, _look_backs: Vec<Vec<f32>>, _times: Vec<f32>) -> f32 {
        0.0
    }
}

// [ ] TODO implement model functionality
pub fn parse_model(
    mtype: StarModelType,
    _mfile: String,
) -> Box<dyn StarModel + Send> {
    match mtype {
        StarModelType::None => Box::new(NoneModel {}),
    }
}
