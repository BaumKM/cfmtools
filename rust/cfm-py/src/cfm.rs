use std::sync::Arc;
use std::time::Duration;

use cfm_core::algorithms::general::sampling::UniformRankingSampler;
use cfm_core::algorithms::structural::sampling::UniformBacktrackingSampler;
use cfm_core::algorithms::{EnumerationStatus, MaybeDuration, UnconstrainedSummary};
use cfm_core::benchmarks::structural::{BacktrackingBenchmark, RankingBenchmark};
use cfm_core::benchmarks::{
    Benchmark, BenchmarkParams, BenchmarkResult, RunResult, RuntimeResult, UniformityResult,
};
use cfm_core::config_spaces::structural::StructuralConfigSpace;
use cfm_core::model::feature::{Feature, FeatureVec};
use cfm_core::utils::data_structures::Tree;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyList};

use cfm_core::model::cfm::CFM;
use rug::Integer;
use serde::Serialize;
use serde_json::{Value, json};

use crate::cfm::convert::build_py_cfm_from_bytes;

mod convert;

/// Python-visible wrapper around the Rust CFM.
/// This type owns the compiled Rust model.
#[pyclass(name = "CFM", frozen)]
pub struct PyCfm {
    inner: Arc<CFM>,
}

#[pyclass(name = "StructuralCFM", frozen)]
pub struct PyStructuralCfm {
    cfm: Arc<CFM>,
}

#[pymethods]
impl PyCfm {
    /// Build a native CFM from serialized bytes (JSON).
    #[staticmethod]
    pub fn from_bytes<'py>(py: Python<'py>, data: &Bound<'py, PyBytes>) -> PyResult<Self> {
        let bytes = data.as_bytes();

        let cfm = py
            .detach(|| build_py_cfm_from_bytes(bytes))
            .map_err(PyErr::new::<pyo3::exceptions::PyValueError, _>)?;

        Ok(Self {
            inner: Arc::new(cfm),
        })
    }
    pub fn structural(&self) -> PyStructuralCfm {
        PyStructuralCfm {
            cfm: self.inner.clone(),
        }
    }
}

#[pymethods]
impl PyStructuralCfm {
    /// Benchmarks uniform ranking sampling quality + runtime.
    #[pyo3(signature = (runs, samples, seed, calculate_constrained_space_size))]
    pub fn benchmark_ranking_sampler<'py>(
        &self,
        py: Python<'py>,
        runs: usize,
        samples: usize,
        seed: u64,
        calculate_constrained_space_size: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let config_space = StructuralConfigSpace::new(self.cfm.clone());
        let sampler = UniformRankingSampler::new(self.cfm.clone(), config_space);
        let bench = RankingBenchmark { sampler };

        run_benchmark(
            py,
            self.cfm.clone(),
            bench,
            runs,
            samples,
            seed,
            calculate_constrained_space_size,
        )
    }

    /// Benchmarks uniform backtracking sampling quality + runtime.
    #[pyo3(signature = (runs, samples, seed, calculate_constrained_space_size))]
    pub fn benchmark_backtracking_sampler<'py>(
        &self,
        py: Python<'py>,
        runs: usize,
        samples: usize,
        seed: u64,
        calculate_constrained_space_size: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let config_space = StructuralConfigSpace::new(self.cfm.clone());
        let sampler = UniformBacktrackingSampler::new(config_space);
        let bench = BacktrackingBenchmark { sampler };

        run_benchmark(
            py,
            self.cfm.clone(),
            bench,
            runs,
            samples,
            seed,
            calculate_constrained_space_size,
        )
    }

    /// Summarize the unconstrained configuration space.
    #[pyo3(signature = (show_full_tree = false))]
    pub fn unconstrained_config_space_summary<'py>(
        &self,
        py: Python<'py>,
        show_full_tree: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value: Value = py.detach(|| {
            let cs = StructuralConfigSpace::new(self.cfm.clone());

            let summary = cs.summarize_unconstrained();

            let mut json_root = serde_json::Map::new();

            // Feature tree
            if show_full_tree {
                let feature_tree = build_feature_tree_json(&self.cfm, &summary);
                json_root.insert("feature tree".into(), feature_tree);
            }

            // Tree summary
            json_root.insert(
                "tree_summary".into(),
                serde_json::to_value(&summary.tree_summary).expect("tree summary is serializable"),
            );

            let root = self.cfm.root();

            // Unconstrained config space
            json_root.insert(
                "unconstrained".into(),
                json!({
                    "number_of_cross_tree_constraints":
                        summary.number_of_cross_tree_constraints,

                    "total_configurations":
                        summary.config_counts[root].to_string(),

                    "avg_configuration_size":
                        summary.avg_config_sizes[root],
                }),
            );

            // Build timings
            json_root.insert(
                "build_times".into(),
                json!({
                    "count_dp_build_time_us":
                        format_duration_us(&summary.count_dp_build_time),

                    "size_dp_build_time_us":
                        format_duration_us(&summary.size_dp_build_time),
                }),
            );

            Value::Object(json_root)
        });

        json_to_py(py, &value)
    }

    /// Summarize constrained enumeration with a time limit.
    #[pyo3(signature = (time_limit_s, show_rank_validity = false))]
    pub fn constrained_config_space_summary<'py>(
        &self,
        py: Python<'py>,
        time_limit_s: u64,
        show_rank_validity: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value: Value = py.detach(|| {
            let cs = StructuralConfigSpace::new(self.cfm.clone());

            let summary =
                cs.enumerate_constrained(Duration::from_secs(time_limit_s), show_rank_validity);

            let mut json_root = serde_json::Map::new();

            // Optional rank validity
            if show_rank_validity && let Some(validity) = summary.rank_cross_tree_validity {
                json_root.insert(
                    "rank_cross_tree_validity".into(),
                    serde_json::to_value(validity).expect("vec is serializable"),
                );
            }

            // Constrained enumeration data
            json_root.insert(
                "constrained".into(),
                json!({
                    "time_limit_s": summary.time_limit.as_secs(),

                    "enumerated_configurations": summary.enumerated,

                    "valid_configurations": summary.valid,

                    "valid_ratio": summary.valid_ratio,

                    "avg_configuration_size": summary.avg_valid_size,

                    "time_to_first_valid_us":
                        summary
                            .time_to_first_valid
                            .map(|d| format_duration_us(&d)),

                    "status":
                        enumeration_status_to_json(&summary.status),
                }),
            );

            // Build timings
            json_root.insert(
                "build_times".into(),
                json!({
                    "count_dp_build_time_us":
                        format_duration_us(&summary.count_dp_build_time),
                }),
            );

            Value::Object(json_root)
        });

        json_to_py(py, &value)
    }
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCfm>()?;
    m.add_class::<PyStructuralCfm>()?;
    Ok(())
}

fn json_to_py<'py>(py: Python<'py>, v: &Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match v {
        Value::Null => py.None().into_bound(py),

        Value::Bool(b) => PyBool::new(py, *b).to_owned().into_any(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)?.into_any()
            } else if let Some(u) = n.as_u128() {
                u.into_pyobject(py)?.into_any()
            } else if let Some(f) = n.as_f64() {
                f.into_pyobject(py)?.into_any()
            } else {
                n.to_string().into_pyobject(py)?.into_any()
            }
        }

        Value::String(s) => s.as_str().into_pyobject(py)?.into_any(),

        Value::Array(arr) => {
            let py_list = PyList::empty(py);
            for x in arr {
                py_list.append(json_to_py(py, x)?)?;
            }
            py_list.into_any()
        }

        Value::Object(obj) => {
            let py_dict = PyDict::new(py);
            for (k, x) in obj {
                py_dict.set_item(k, json_to_py(py, x)?)?;
            }
            py_dict.into_any()
        }
    })
}

fn build_feature_tree_json(cfm: &CFM, summary: &UnconstrainedSummary) -> Value {
    let root_feature = cfm.root();

    build_node_json(
        cfm,
        root_feature,
        &summary.config_counts,
        &summary.avg_config_sizes,
    )
}

fn build_node_json(
    cfm: &CFM,
    feature: &Feature,
    config_counts_unconstrained: &FeatureVec<Integer>,
    avg_sizes_unconstrained: &FeatureVec<f64>,
) -> Value {
    let mut children: Vec<Value> = cfm
        .children(feature)
        .map(|child| {
            build_node_json(
                cfm,
                child,
                config_counts_unconstrained,
                avg_sizes_unconstrained,
            )
        })
        .collect();

    children.sort_by(|a, b| {
        let na = a["feature"].as_str().unwrap_or("");
        let nb = b["feature"].as_str().unwrap_or("");
        na.cmp(nb)
    });

    let name = cfm.feature_name(feature).name();

    let total_config_count = config_counts_unconstrained[feature].clone();
    let avg_size = avg_sizes_unconstrained[feature];

    json!({
        "feature": name,
        "total_config_count": total_config_count.to_string(),
        "avg_config_size": avg_size,
        "children": children,
    })
}

fn enumeration_status_to_json(status: &EnumerationStatus) -> Value {
    match status {
        EnumerationStatus::Finished { enumeration_time } => {
            json!({
                "state": "finished",
                "enumeration_time_us": enumeration_time.as_micros(),
            })
        }
        EnumerationStatus::Incomplete {
            enumeration_time,
            estimated_enumeration_time,
        } => {
            json!({
                "state": "incomplete",
                "enumeration_time_us": format_duration_us(enumeration_time),
                "estimated_enumeration_time_us":
                    format_maybe_duration_us(estimated_enumeration_time),
            })
        }
    }
}

fn format_maybe_duration_us(d: &MaybeDuration) -> String {
    match d {
        MaybeDuration::Finite(d) => format_duration_us(d),
        MaybeDuration::Infinite => "infinite".to_string(),
    }
}

fn format_duration_us(duration: &Duration) -> String {
    duration.as_micros().to_string()
}

fn run_benchmark<B: Benchmark>(
    py: Python<'_>,
    cfm: Arc<CFM>,
    bench: B,
    runs: usize,
    samples: usize,
    seed: u64,
    calculate_constrained_space_size: bool,
) -> PyResult<Bound<'_, PyAny>> {
    let params = BenchmarkParams {
        runs,
        samples,
        seed,
        calculate_constrained_space_size,
    };

    let json: Value = py.detach(|| {
        let result = bench.run(&cfm, &params);
        benchmark_result_to_json(&result)
    });

    json_to_py(py, &json)
}

fn benchmark_result_to_json<S: Serialize>(result: &BenchmarkResult<S>) -> Value {
    json!({
        "runs": result.runs.iter().map(run_result_to_json).collect::<Vec<_>>(),
    })
}

fn run_result_to_json<S: Serialize>(run: &RunResult<S>) -> Value {
    json!({
        "runtime": runtime_result_to_json(&run.runtime),
        "uniformity": uniformity_result_to_json(&run.uniformity),
        "sampler_stats": run.sampler_stats,
    })
}

fn runtime_result_to_json(runtime: &RuntimeResult) -> Value {
    json!({
        "setup_time": format_duration_us(&runtime.setup_time),
        "sampling_time": format_duration_us(&runtime.sampling_time),
        "ranking_time": format_duration_us(&runtime.ranking_time),
        "total_time": format_duration_us(&
            (runtime.setup_time + runtime.sampling_time + runtime.ranking_time)),
    })
}

fn uniformity_result_to_json(u: &UniformityResult) -> Value {
    match u {
        UniformityResult::KnownSupport {
            constrained_space_size,
            samples,
            distribution,
            chi_square,
            chi_square_pvalue,
            total_variation,
            max_deviation,
        } => json!({
            "support": "known",
            "constrained_space_size": constrained_space_size.to_string(),
            "samples": samples,

            "chi_square": chi_square.to_string(),
            "chi_square_pvalue": chi_square_pvalue.to_string(),
            "total_variation": total_variation.to_string(),
            "max_deviation": max_deviation.to_string(),

            "distribution": distribution.iter().map(|(rank, count)| {
                json!([rank.to_string(), count])
            }).collect::<Vec<_>>(),
        }),

        UniformityResult::UnknownSupport {
            samples,
            p_max,
            collision_probability,
            effective_bins,
            distribution,
        } => json!({
            "support": "unknown",
            "samples": samples,

            "p_max": p_max,
            "collision_probability": collision_probability,
            "effective_bins": effective_bins,

            "distribution": distribution.iter().map(|(value, count)| {
                json!([value.to_string(), count])
            }).collect::<Vec<_>>(),
        }),
    }
}
