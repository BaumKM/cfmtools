use pyo3::{Bound, PyResult, Python, pymodule, types::PyModule};

mod cfm;

#[pymodule]
fn _cfm_native(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    cfm::register(py, m)
}
