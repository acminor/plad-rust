use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use crate::star::{StarModel, StarModelErr};
use std::cell::RefCell;

pub struct NNPPredictor<'a> {
    predictor: &'a pyo3::types::PyAny, // used for predictor value
    py: RefCell<Python<'a>>
}

impl <'a> NNPPredictor<'a> {
    pub fn new(py: RefCell<Python<'a>>,
               args: std::collections::HashMap<String, String>)
           -> NNPPredictor {
        let p2 = *py.borrow();
        let look_back = {
            let lb = args.get("look_back").unwrap();

            lb.parse::<u32>().unwrap()
        };
        let model_file = args.get("arima_model_file").unwrap();

        let syspath: &pyo3::types::PyList = p2.import("sys")
            .unwrap()
            .get("path")
            .unwrap()
            .try_into()
            .unwrap();

        syspath.insert(0, "./").unwrap();

        let lstm = match p2.import("lstm") {
            Ok(ok) => {ok},
            Err(err) => {err.print(p2); panic!()}
        };

        let predictor = match p2.eval("lstm.LSTMP(1)",
                                      None,
                                      Some([("lstm", lstm)]
                                           .into_py_dict(p2)), ) {
            Ok(ok) => {
                match p2.eval("predictor.load_model_weights",
                              None,
                              Some([("predictor", ok)].into_py_dict(p2))) {
                    Ok(_) => { ok },
                    Err(err) => {err.print(p2); panic!()}
                }
            },
            Err(err) => {err.print(p2); panic!()}
        };

        //let locals = [("predictor", predictor)].into_py_dict(p2);

        NNPPredictor{predictor, py}
    }
}

impl StarModel for NNPPredictor<'_> {
    fn init(&self, args: std::collections::HashMap<String, String>)
            -> StarModelErr
    {
        Ok(())
    }
    fn predict(&self,
               look_backs: Vec<Vec<f32>>, // current value plus previous values
               times: Vec<f32>) -> f32
    {
        let py = *self.py.borrow();

        let look_backs = pyo3::types::PyList::new(py, look_backs);
        let times = pyo3::types::PyList::new(py, times);

        let locals = [("predictor", self.predictor),
                     ("look_backs", look_backs.as_ref()),
                     ("times", times.as_ref())].into_py_dict(py);

        let result = match py.eval("predictor.predict(look_backs)",
                                   None, Some(locals), ) {
            Ok(ok) => {},
            Err(err) => {err.print(py); panic!()}
        };

        0.0
    }
}
