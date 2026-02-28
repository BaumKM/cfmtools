import inspect
import json
from pathlib import Path
from typing import Annotated, Any, override

import statistics

from cfmtools.core.cfm import CFM, CardinalityInterval
from cfmtools.pipeline.analyze import Analyzer
from cfmtools.pipeline.core import ParamHelp
from cfmtools.pluginsystem import analyze


def _tree_height(model: CFM) -> int:
    max_depth = 0
    stack = [(model.root, 0)]

    while stack:
        node, depth = stack.pop()
        max_depth = max(max_depth, depth)
        for child in model.children[node]:
            stack.append((child, depth + 1))

    return max_depth


def _finite_cardinality_features(
    cards: list[CardinalityInterval],
) -> dict[str, int | float]:

    lowers: list[int] = []
    uppers: list[int] = []
    sizes: list[int] = []

    for c in cards:
        if c.max is None or c.size is None:
            raise ValueError("Unbounded cardinality interval encountered.")

        lowers.append(c.min)
        uppers.append(c.max)
        sizes.append(c.size)

    return {
        # --- lower bounds ---
        "lower_min": min(lowers) if lowers else 0,
        "lower_median": statistics.median(lowers) if lowers else 0,
        "lower_mean": statistics.mean(lowers) if lowers else 0.0,
        "lower_max": max(lowers) if lowers else 0,
        # --- upper bounds ---
        "upper_median": statistics.median(uppers) if uppers else 0,
        "upper_mean": statistics.mean(uppers) if uppers else 0.0,
        "upper_max": max(uppers) if uppers else 0,
        # --- interval sizes ---
        "size_min": min(sizes) if sizes else 0,
        "size_median": statistics.median(sizes) if sizes else 0,
        "size_mean": statistics.mean(sizes) if sizes else 0.0,
        "size_max": max(sizes) if sizes else 0,
    }


def _dp_runtime_estimator(model: CFM) -> int:
    n_features = model.n_features

    max_gi = max(c.max for c in model.group_instance_cardinalities if c.max is not None)
    max_gt = max(c.max for c in model.group_type_cardinalities if c.max is not None)
    max_fi_size = max(
        c.size for c in model.feature_instance_cardinalities if c.size is not None
    )

    return n_features * max_gi * max_gt * max_fi_size


def _total_instance_bound(model: CFM) -> int:
    """
    Sum of per-feature instance upper bounds.
    """
    bounds = model.compute_instance_bounds()
    return sum(bounds)


@analyze("feature-tree")
class FeatureTreeAnalyzer(Analyzer):
    """
    Compute a tree and cardinality summary of a CFM.

    Produces a JSON report capturing tree structure, cross-tree
    constraints, cardinality distributions, and derived complexity
    indicators.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Export a structural feature-tree summary as JSON."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Compute a feature tree and cardinality summary of the current CFM
            and export the result as a JSON report.

            The generated report contains:

              Tree metrics:
                - number of features, leaves, and internal nodes
                - tree height
                - average and maximum branching factors

              Cross-tree constraints:
                - number of require and exclude constraints
                - constraint densities relative to the number of
                  possible constraints

              Cardinality statistics:
                - summaries of lower and upper bounds
                - interval size statistics
                - per-category aggregation (feature-instance,
                  group-instance, group-type)

              Derived properties:
                - total instance upper bound
                - dynamic-programming runtime estimator

            This analyzer does not modify the model.
        """)

    def __init__(
        self,
        output_path: Annotated[
            Path,
            ParamHelp(
                "Path to the output JSON file where the feature-tree summary are written."
            ),
        ],
    ) -> None:
        self.output_path = output_path

    @override
    def analyze(self, model: CFM) -> None:
        n_features = model.n_features

        # --------------------------------------------------------
        # Tree structure
        # --------------------------------------------------------

        children_counts = [len(model.children[f]) for f in model.features()]
        n_leaves = sum(1 for c in children_counts if c == 0)
        n_internal = n_features - n_leaves

        non_leaf_branching = [c for c in children_counts if c > 0]

        tree_features: dict[str, float | int] = {
            "n_features": n_features,
            "n_leaves": n_leaves,
            "n_internal": n_internal,
            "tree_height": _tree_height(model),
            "avg_internal_branching_factor": (
                sum(non_leaf_branching) / len(non_leaf_branching)
                if non_leaf_branching
                else 0.0
            ),
            "max_branching_factor": max(children_counts) if children_counts else 0,
        }

        # --------------------------------------------------------
        # Cross-tree constraints
        # --------------------------------------------------------

        n_require = len(model.require_constraints)
        n_exclude = len(model.exclude_constraints)
        n_cross = n_require + n_exclude

        req_possible = n_features * (n_features - 1)  # directed, no self-loops
        exc_possible = n_features * (n_features - 1) // 2  # undirected, no self-loops

        require_density = (n_require / req_possible) if req_possible > 0 else 0.0
        exclude_density = (n_exclude / exc_possible) if exc_possible > 0 else 0.0

        combined_possible = req_possible + exc_possible
        cross_tree_density = (
            ((n_require + n_exclude) / combined_possible)
            if combined_possible > 0
            else 0.0
        )

        constraint_features: dict[str, float | int] = {
            "n_require_constraints": n_require,
            "n_exclude_constraints": n_exclude,
            "n_cross_tree_constraints": n_cross,
            "require_density": require_density,
            "exclude_density": exclude_density,
            "cross_tree_constraint_density": cross_tree_density,
        }

        # --------------------------------------------------------
        # Cardinalities
        # --------------------------------------------------------

        cardinality_features = {
            "feature_instance": _finite_cardinality_features(
                list(model.feature_instance_cardinalities)
            ),
            "group_instance": _finite_cardinality_features(
                list(model.group_instance_cardinalities)
            ),
            "group_type": _finite_cardinality_features(
                list(model.group_type_cardinalities)
            ),
        }

        # --------------------------------------------------------
        # Derived
        # --------------------------------------------------------

        derived_properties = {
            "total_instance_bound": _total_instance_bound(model),
            "dp_runtime_estimator": _dp_runtime_estimator(model),
        }

        result: dict[str, Any] = {
            "tree": tree_features,
            "constraints": constraint_features,
            "cardinalities": cardinality_features,
            "derived": derived_properties,
        }

        self.output_path.parent.mkdir(parents=True, exist_ok=True)
        with self.output_path.open("w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)
