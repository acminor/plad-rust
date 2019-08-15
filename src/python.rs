use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyAny};
use std::cell::RefCell;
use std::marker::PhantomData;

use rental::common::RentRef;
use owning_ref::OwningRef;

pub fn build_tf_ref<'a>(py: RefCell<Python<'a>>) -> RefCell<&'a PyAny> {
    RefCell::new([("tf", "")].into_py_dict(*py.borrow()).as_ref())
}

pub fn build_np_ref<'a>(py: RefCell<Python<'a>>) -> RefCell<&'a PyAny> {
    RefCell::new([("np", "")].into_py_dict(*py.borrow()).as_ref())
}

type PythonInstance<'a> = RentRef<Box<GILGuard>, RefCell<Python<'a>>>;
pub fn build_py_ref<'a>(_: PhantomData<&'a i32>) -> PythonInstance<'a> {
    let gil = Box::new(Python::acquire_gil());
    let or = RentRef::new(gil, |gil| &RefCell::new(gil.python()));

    or
}
