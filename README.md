# CFMTools

![Python](https://img.shields.io/badge/python-%E2%89%A53.14-blue)
![Platform](https://img.shields.io/badge/platform-linux-lightgrey)
![License](https://img.shields.io/badge/license-MIT-green)
![Rust](https://img.shields.io/badge/backend-rust-orange)

**CFMTools** is an algorithmic framework for **Cardinality-Based Feature Models (CFMs)**.

It supports importing, transforming, analyzing, summarizing, and **uniform random sampling** of CFMs under:

- **Structural configuration semantics**
- **Semi-structural configuration semantics**

> Structural and semi-structural configuration semantics are introduced in  
> *(Thesis link placeholder — to be released)*

---

# Features

## 1. Import & Export

### Import

- **UVL (Boolean core only)**  
  Supports most of the Boolean core of [Universal Variability Language (UVL)](https://universal-variability-language.github.io/).  
  Not supported: numeric constraints, arithmetic expressions, or complex propositional formulas.

- **JSON**

### Export

- **JSON**

- Pretty-printed **stdout**


## 2. Model Transformations

- **Big-M Bounding**  
  Transforms a CFM with an infinite configuration space into one with a finite configuration space while preserving all non-convex regions.

- **Elimination of Trivial Dead Cardinalities**  
  Removes unreachable cardinalities without changing valid configurations.


## 3. Configuration Space Analysis

CFMTools distinguishes between:

- **Unconstrained configuration space**  
  (ignoring cross-tree constraints)

- **Constrained configuration space**  
  (including cross-tree constraints)


### 3.1 Structural Semantics (DP-based)

Uses dynamic programming to calculate the number of structural configurations.

Supported summaries:

#### Unconstrained Summary
- Total number of configurations (per feature and root)
- Average configuration size (per feature and root)

#### Constrained Analysis (Time-Bounded Enumeration)
- Total number of valid configurations (if complete)
- Average configuration size
- Enumerated configurations
- Valid ratio (valid / enumerated)

#### Performance Metrics
- DP build time
- Enumeration time
- Time to first configuration

### 3.2 Semi-Structural Semantics (CP-SAT / OR-Tools)

CFMs are translated into a CP-SAT model (OR-Tools).

Summary includes:
- Solver status
- Number of configurations
- Time to first configuration


## 4. Sampling (Structural Semantics)

Currently implemented:

- **Uniform Ranking Sampler**  
  DP-based ranking/unranking. Exact uniform sampling.

- **Uniform Backtracking Sampler**  
  DP-guided structural backtracking. Exact uniform sampling.

Both support:
- Benchmark mode
- Multiple runs
- Configurable random seed

---

# Architecture

CFMTools is a **Python CLI pipeline** with a Rust backend.  
Pipeline stages are extensible via dynamically discovered plugins.


## Python Frontend

Built on `argparse` and structured as a configurable pipeline:

- `load`
- `transform`
- `analyze`
- `sample`
- `export`

Rules:
- `load` must appear first and exactly once.

General structure:

    cfmtools [GLOBAL OPTIONS] STEP [ARGS] [STEP [ARGS] ...]

Custom commands can be registered via Python entry points:

    cfmtools.plugins


## Rust Backend

Performance-critical components are implemented in Rust and exposed in:

    cfmtools.core._cfm_native

---

# Requirements

- **Python ≥ 3.14**
- **Linux**

Optional:
- `ortools` (semi-structural SAT analysis)
- `uvlparser` (UVL import)

> Currently Linux-only due to GMP in the Rust backend.

---

# Installation

## Option 1 — Install Prebuilt Wheel

Download the prebuilt wheel from the GitHub Releases page and install it with `pip`.


## Option 2 — Build from Source

Requirements:
- Rust (GNU toolchain)
- `maturin`

From the `python/` directory run:

    maturin build --release

### Optional: Enable CPU-Specific Optimizations

For local builds, you can enable CPU-specific optimizations to maximize performance on your machine:

    RUSTFLAGS="-C target-cpu=native" maturin build --release

This enables architecture-specific instruction sets supported by your CPU and may significantly improve performance for compute-intensive workloads.


---

# Usage

CFMTools is a command-line application structured as a pipeline.

General form:

    cfmtools [GLOBAL OPTIONS] STEP [ARGS] [STEP [ARGS] ...]

To see the list of available pipeline steps and global options:

    cfmtools --help

Each pipeline step provides its own help page:

    cfmtools STEP --help

Individual commands within a pipeline step also provide help:

    cfmtools STEP COMMAND --help

Example:

    cfmtools load json --help

---

# Future Work

The following extensions are planned:

## 1. Extended Import / Export Support

- Support additional input formats.
- Support additional export formats.

## 2. DP-Based Extension to Semi-Structural Semantics

- Extend the dynamic programming–based approach to semi-structural configuration semantics for sampling and analysis.

## 3. DP-Based Extension to Instance-Based Semantics

- Extend the dynamic programming–based approach to instance-based configuration semantics for sampling and analysis.