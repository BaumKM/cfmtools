import inspect
import json
from pathlib import Path
import time
from typing import Annotated, override

from ortools.sat.python import cp_model
from ortools.sat.python.cp_model import CpModel, IntVar

from cfmtools.core.cfm import CFM, JSON, CardinalityInterval, FeatureList
from cfmtools.pipeline.analyze import Analyzer
from cfmtools.pipeline.core import ParamHelp
from cfmtools.pluginsystem import analyzer


def _enforce_cardinality_when_active(
    model: CpModel,
    x: IntVar,
    active: IntVar,
    card: CardinalityInterval,
    name: str,
) -> None:
    """
    Enforce:
      - if active == 0: x == 0
      - if active == 1: x ∈ card
    """

    # Inactive -> zero
    model.add(x == 0).only_enforce_if(active.Not())  # type: ignore

    intervals = list(card)

    selectors: list[IntVar] = []
    for k, iv in enumerate(intervals):
        sel = model.new_bool_var(f"{name}_sel_{k}")
        selectors.append(sel)

        # If this interval is selected, x must lie inside it
        model.add(x >= iv.lower).only_enforce_if(sel)  # type: ignore
        if iv.upper is not None:
            model.add(x <= iv.upper).only_enforce_if(sel)  # type: ignore

    # active = 1  <=>  exactly one interval is selected
    model.add(sum(selectors) == active)


def _in_cardinality_interval_bool(
    model: CpModel,
    x: IntVar,
    card: CardinalityInterval,
    name: str,
) -> IntVar:
    """
    Returns b ∈ {0,1} such that:
        b == 1  <=>  x ∈ card
    """
    b = model.new_bool_var(f"{name}_in")
    intervals = list(card)

    selectors: list[IntVar] = []

    for k, iv in enumerate(intervals):
        sel = model.new_bool_var(f"{name}_sel_{k}")
        selectors.append(sel)

        # sel = 1 => x ∈ I_k
        model.add(x >= iv.lower).only_enforce_if(sel)  # type: ignore
        if iv.upper is not None:
            model.add(x <= iv.upper).only_enforce_if(sel)  # type: ignore

        # sel = 0 => x ∉ I_k
        # i.e., x < lower OR x > upper
        if iv.upper is not None:
            below = model.new_bool_var(f"{name}_below_{k}")
            above = model.new_bool_var(f"{name}_above_{k}")

            model.add(x < iv.lower).only_enforce_if(below)  # type: ignore
            model.add(x > iv.upper).only_enforce_if(above)  # type: ignore

            # if sel = 0, one of below/above must hold
            model.add_bool_or([below, above]).only_enforce_if(sel.Not())  # type: ignore
        else:
            # Interval [lower, +∞)
            below = model.new_bool_var(f"{name}_below_{k}")
            model.add(x < iv.lower).only_enforce_if(below)  # type: ignore
            model.add(below == 1).only_enforce_if(sel.Not())  # type: ignore

    # Exactly one interval selected iff b = 1
    model.add(sum(selectors) == b)

    return b


def _max_cardinality(card: CardinalityInterval, *, what: str) -> int:
    """
    Return the finite maximum of a cardinality interval.
    """
    max_value = card.max
    if max_value is None:
        raise ValueError(
            f"Cannot build hierarchical CP-SAT cloning encoding: "
            f"{what} has an unbounded cardinality interval ({card})."
        )
    return max_value


def cfm_to_cp_sat(
    cfm: CFM,
) -> tuple[CpModel, FeatureList[list[IntVar]], FeatureList[list[tuple[int, int]]]]:
    model: CpModel = CpModel()
    name = cfm.feature_name

    # ------------------------------------------------------------
    # Variables
    # ------------------------------------------------------------

    # instance_vars[f] = flat list of all potential instances of feature f
    instance_vars: FeatureList[list[IntVar]] = FeatureList(
        [[] for _ in range(cfm.n_features)]
    )

    # instance_offsets[f][p_i] = (start, end)
    # feature instances that belong under i'th instance of p
    # i.e., instance_vars[f][start:end] are exactly those child instances
    instance_offsets: FeatureList[list[tuple[int, int]]] = FeatureList(
        [[] for _ in range(cfm.n_features)]
    )

    root = cfm.root
    root_var = model.new_bool_var(f"instance_{name(root)}_0")
    instance_vars[root] = [root_var]
    instance_offsets[root] = [(0, 1)]

    for feature in cfm.traverse_preorder():
        parent_feature = cfm.parents[feature]
        if parent_feature is None:
            continue

        max_per_parent = _max_cardinality(
            cfm.feature_instance_cardinalities[feature],
            what=f"feature {name(feature)}",
        )

        for parent_i in range(len(instance_vars[parent_feature])):
            start = len(instance_vars[feature])

            for _ in range(max_per_parent):
                instance_index = len(instance_vars[feature])
                instance_vars[feature].append(
                    model.new_bool_var(f"instance_{name(feature)}_{instance_index}")
                )

            end = len(instance_vars[feature])
            instance_offsets[feature].append((start, end))

    # ------------------------------------------------------------
    # Feature Tree constraints
    # ------------------------------------------------------------

    # Root must always be selected.
    model.add(root_var == 1)

    # A child instance may only be active if its parent instance is active.
    for feature in cfm.features():
        parent_feature = cfm.parents[feature]
        if parent_feature is None:
            continue

        for parent_i, (start, end) in enumerate(instance_offsets[feature]):
            parent_active = instance_vars[parent_feature][parent_i]

            for i in range(start, end):
                model.add(instance_vars[feature][i] <= parent_active)

    # ------------------------------------------------------------
    # Cardinality constraints
    # ------------------------------------------------------------

    # For each potential parent instance p_i:
    #   (1) each child feature count under p_i must satisfy feature-instance cardinality
    #   (2) total child count must satisfy group-instance cardinality of p
    #   (3) number of distinct child types present must satisfy group-type cardinality of p
    for parent_feature in cfm.features():
        children = cfm.children[parent_feature]

        for parent_i in range(len(instance_vars[parent_feature])):
            parent_instance = instance_vars[parent_feature][parent_i]

            instances_per_child: list[IntVar] = []
            present_per_child: list[IntVar] = []

            for child_feature in children:
                start, end = instance_offsets[child_feature][parent_i]
                child_vars = instance_vars[child_feature][start:end]

                # feature_instance_c_i
                child_count = model.new_int_var(
                    0,
                    len(child_vars),
                    f"feature_instance_{name(child_feature)}_{parent_i}",
                )

                model.add(child_count == sum(child_vars))

                _enforce_cardinality_when_active(
                    model,
                    child_count,
                    parent_instance,
                    cfm.feature_instance_cardinalities[child_feature],
                    name=f"fi_card_{name(child_feature)}_{parent_i}",
                )

                instances_per_child.append(child_count)

                present = model.new_bool_var(
                    f"present_{name(child_feature)}_{parent_i}"
                )

                if child_vars:
                    model.add_max_equality(present, child_vars)
                else:
                    model.add(present == 0)

                present_per_child.append(present)

            # group_instance_p_i
            group_instances = model.new_int_var(
                0,
                sum(
                    instance_offsets[c][parent_i][1] - instance_offsets[c][parent_i][0]
                    for c in children
                ),
                f"group_instance_{name(parent_feature)}_{parent_i}",
            )

            model.add(group_instances == sum(instances_per_child))

            _enforce_cardinality_when_active(
                model,
                group_instances,
                parent_instance,
                cfm.group_instance_cardinalities[parent_feature],
                name=f"gi_{name(parent_feature)}_{parent_i}",
            )

            # group_type_p_i
            group_types = model.new_int_var(
                0,
                len(children),
                f"group_type_{name(parent_feature)}_{parent_i}",
            )

            model.add(group_types == sum(present_per_child))

            _enforce_cardinality_when_active(
                model,
                group_types,
                parent_instance,
                cfm.group_type_cardinalities[parent_feature],
                name=f"gt_card_{name(parent_feature)}_{parent_i}",
            )

    # ------------------------------------------------------------
    # Cross-tree constraints
    # ------------------------------------------------------------

    # Global count of instances per feature: feature_count[f] = number of active instances of f
    feature_count: FeatureList[IntVar] = FeatureList(
        [
            model.new_int_var(0, len(instance_vars[f]), f"cnt_{name(f)}")
            for f in cfm.features()
        ]
    )

    for feature in cfm.features():
        model.add(feature_count[feature] == sum(instance_vars[feature]))

    # Require
    for k, rc in enumerate(cfm.require_constraints):
        a = rc.first_feature
        b = rc.second_feature

        a_in: IntVar = _in_cardinality_interval_bool(
            model,
            feature_count[a],
            rc.first_cardinality,
            name=f"req{k}_{name(a)}",
        )
        b_in: IntVar = _in_cardinality_interval_bool(
            model,
            feature_count[b],
            rc.second_cardinality,
            name=f"req{k}_{name(b)}",
        )

        model.add_implication(a_in, b_in)

    # Exclude
    for k, ec in enumerate(cfm.exclude_constraints):
        a = ec.first_feature
        b = ec.second_feature

        a_in = _in_cardinality_interval_bool(
            model,
            feature_count[a],
            ec.first_cardinality,
            name=f"ex{k}_{name(a)}",
        )
        b_in = _in_cardinality_interval_bool(
            model,
            feature_count[b],
            ec.second_cardinality,
            name=f"ex{k}_{name(b)}",
        )

        model.add_bool_or([a_in.Not(), b_in.Not()])

    # ------------------------------------------------------------
    # Symmetry breaking
    # ------------------------------------------------------------

    # Dense usage of child instances under each parent instance
    for feature in cfm.features():
        parent_feature = cfm.parents[feature]
        if parent_feature is None:
            continue

        for parent_i, (start, end) in enumerate(instance_offsets[feature]):
            for i in range(start, end - 1):
                model.add(instance_vars[feature][i + 1] <= instance_vars[feature][i])

    return model, instance_vars, instance_offsets


@analyzer("semi-structural-sat")
class ConfigurationSpaceSummary(Analyzer):

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Summarize constrained semi-structural configuration space."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Translate the current CFM into a CP-SAT model and
            enumerate valid configurations within a time limit.

            The generated JSON report includes:

              - solver status and completeness
              - number of solutions found
              - time to convert CFM -> SAT
              - time to first solution
              - total solver wall time
              - number of SAT variables
              - total instance variables
              - solver configuration (threads, seed)

            The enumeration stops when either:
              - the solver proves optimality (complete enumeration), or
              - the specified time limit is reached.

            This analyzer does not modify the model.
        """)

    def __init__(
        self,
        output_path: Annotated[
            Path,
            ParamHelp("File path where solver output will be written."),
        ],
        time_limit: Annotated[
            int,
            ParamHelp(
                "Maximum solving time in seconds. The solver stops once this limit is reached."
            ),
        ],
        threads: Annotated[
            int,
            ParamHelp(
                "Number of parallel search workers (CP-SAT num_search_workers). "
                "Use 1 for deterministic behavior. "
                "Values >1 enable parallel search but may introduce nondeterminism."
            ),
        ] = 1,
        seed: Annotated[
            int,
            ParamHelp(
                "Random seed passed to the solver (CP-SAT random_seed). "
                "Controls randomized search components such as LNS and branching. "
                "Use a fixed value together with threads=1 for reproducible results."
            ),
        ] = 1,
    ) -> None:
        if time_limit <= 0:
            raise ValueError("time_limit must be a positive integer")

        if threads <= 0:
            raise ValueError("threads must be a positive integer")

        self.output_path = output_path
        self.time_limit = time_limit
        self.threads = threads
        self.seed = seed

    @override
    def analyze(self, model: CFM) -> None:
        build_start = time.perf_counter()

        sat_model, instance_vars, _instance_offsets = cfm_to_cp_sat(model)

        build_time_us = int((time.perf_counter() - build_start) * 1_000_000)

        total_instance_variables = sum(len(instance_vars[f]) for f in model.features())

        solver = cp_model.CpSolver()
        solver.parameters.enumerate_all_solutions = True
        solver.parameters.max_time_in_seconds = self.time_limit
        solver.parameters.num_search_workers = self.threads
        solver.parameters.random_seed = self.seed

        stats = _EnumerationStats()
        status = solver.solve(sat_model, stats)
        num_variables = len(sat_model.Proto().variables)  # type: ignore

        result: JSON = {
            "status": solver.status_name(status),
            "solutions_found": stats.solution_count,
            "time_limit_s": self.time_limit,
            "time_to_convert_cfm_to_sat_us": build_time_us,
            "time_to_first_solution_us": stats.first_solution_time_us,
            "total_wall_time_us": int(solver.wall_time * 1_000_000),
            "threads": self.threads,
            "seed": self.seed,
            "complete": status == cp_model.OPTIMAL,
            "num_variables": num_variables,
            "total_instance_variables": total_instance_variables,
        }

        self.output_path.parent.mkdir(parents=True, exist_ok=True)
        with self.output_path.open("w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)


class _EnumerationStats(cp_model.CpSolverSolutionCallback):
    def __init__(self):
        super().__init__()
        self.solution_count = 0
        self.first_solution_time_us = None
        self.start_time = time.perf_counter()

    def on_solution_callback(self):
        self.solution_count += 1

        if self.solution_count == 1:
            elapsed_seconds = time.perf_counter() - self.start_time
            self.first_solution_time_us = int(elapsed_seconds * 1_000_000)
