import inspect
import json
from pathlib import Path
import time
from typing import Annotated, override
from cfmtools.core.cfm import CFM, JSON, CardinalityInterval, FeatureList
from ortools.sat.python.cp_model import CpModel, IntVar
from ortools.sat.python import cp_model

from cfmtools.pipeline.analyze import Analyzer
from cfmtools.pipeline.core import ParamHelp
from cfmtools.pluginsystem import analyze


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


def cfm_to_cp_sat(
    cfm: CFM,
) -> tuple[
    CpModel,
    FeatureList[list[IntVar]],
    FeatureList[list[list[IntVar]]],
    FeatureList[int],
]:
    model: CpModel = CpModel()

    max_instances: FeatureList[int] = cfm.compute_instance_bounds()
    name = cfm.feature_name

    # ------------------------------------------------------------
    # Variables
    # ------------------------------------------------------------

    # instances[f][i] = whether instance i of feature f exists
    instance_vars: FeatureList[list[IntVar]] = FeatureList(
        [[] for _ in range(cfm.n_features)]
    )

    for feature in cfm.features():
        instance_vars[feature] = [
            model.new_bool_var(f"instance_{name(feature)}_{i}")
            for i in range(0, max_instances[feature])
        ]

    # parents[f][i][j] = instance f_i is attached to parent instance p_j
    parent_vars: FeatureList[list[list[IntVar]]] = FeatureList(
        [[] for _ in range(cfm.n_features)]
    )

    new_bool = model.new_bool_var

    for feature in cfm.features():
        parent_instance = cfm.parents[feature]
        n_f = max_instances[feature]
        n_p = max_instances[parent_instance] if parent_instance is not None else 0

        parent_vars[feature] = [
            [new_bool(f"parent_{name(feature)}_{i}_{j}") for j in range(1, n_p + 1)]
            for i in range(1, n_f + 1)
        ]

    # is_present_in_group[p][i][c] = 1 iff under the i'th feature instance of p has,
    # at least one instance of child c
    is_present_in_group: FeatureList[list[list[IntVar]]] = FeatureList(
        [[] for _ in range(cfm.n_features)]
    )

    for parent_feature in cfm.features():
        children = cfm.children[parent_feature]

        for i in range(max_instances[parent_feature]):
            row: list[IntVar] = []

            for child_feature in children:
                present = model.new_bool_var(
                    f"is_present_under_{name(parent_feature)}_{i}_{name(child_feature)}"
                )
                row.append(present)

                if max_instances[child_feature] > 0:
                    model.add_max_equality(
                        present,
                        [
                            parent_vars[child_feature][j][i]
                            for j in range(max_instances[child_feature])
                        ],
                    )
                else:
                    model.add(present == 0)

            is_present_in_group[parent_feature].append(row)

    # ------------------------------------------------------------
    # Structural constraints
    # ------------------------------------------------------------

    # Root must always be selected
    root = cfm.root
    model.add(instance_vars[root][0] == 1)

    # Every active child instance has exactly one parent
    for feature in cfm.features():
        parent_feature = cfm.parents[feature]
        if parent_feature is None:
            continue

        for i in range(max_instances[feature]):
            model.add(
                sum(
                    parent_vars[feature][i][j]
                    for j in range(max_instances[parent_feature])
                )
                == instance_vars[feature][i]
            )

            # Cannot attach to inactive parent
            for j in range(max_instances[parent_feature]):
                model.add(
                    parent_vars[feature][i][j] <= instance_vars[parent_feature][j]
                )

    # ------------------------------------------------------------
    # Cardinality constraints
    # ------------------------------------------------------------

    # For each feature instance p_i,
    # (1) feature instance cardinality: number of child instances of c attached under p_i
    #     must be within feature_instance_cardinalities[c]
    #
    # (2) group instance cardinality: total number of child instances attached under p_i
    #     (across all child features) must be within group_instance_cardinalities[p]
    #
    # (3) group type cardinality: number of distinct child feature types present under p_i
    #     must be within group_type_cardinalities[p]

    for parent_feature in cfm.features():
        children = cfm.children[parent_feature]

        for i in range(max_instances[parent_feature]):
            parent_instance = instance_vars[parent_feature][i]

            # child_instances_per_feature[c_index] = count of child c under p_i
            instances_per_child: list[IntVar] = []

            for child_feature in children:
                count_c_p_i = model.new_int_var(
                    0,
                    max_instances[child_feature],
                    f"feature_instance_{name(child_feature)}_under_{name(parent_feature)}_{i}",
                )

                model.add(
                    count_c_p_i
                    == sum(
                        parent_vars[child_feature][j][i]
                        for j in range(max_instances[child_feature])
                    )
                )

                _enforce_cardinality_when_active(
                    model,
                    count_c_p_i,
                    parent_instance,
                    cfm.feature_instance_cardinalities[child_feature],
                    name=f"fi_card_c{name(child_feature)}_p{name(parent_feature)}_{i}",
                )

                instances_per_child.append(count_c_p_i)

            group_instances = model.new_int_var(
                0,
                sum(max_instances[c] for c in children),
                f"group_instance_{name(parent_feature)}_{i}",
            )

            model.add(group_instances == sum(instances_per_child))

            _enforce_cardinality_when_active(
                model,
                group_instances,
                parent_instance,
                cfm.group_instance_cardinalities[parent_feature],
                name=f"gi_card_{name(parent_feature)}_{i}",
            )

            group_types = model.new_int_var(
                0,
                len(children),
                f"cnt_types_under_{name(parent_feature)}_{i}",
            )

            model.add(group_types == sum(is_present_in_group[parent_feature][i]))

            _enforce_cardinality_when_active(
                model,
                group_types,
                parent_instance,
                cfm.group_type_cardinalities[parent_feature],
                name=f"gt_card_p{name(parent_feature)}_{i}",
            )

    # ------------------------------------------------------------
    # Cross-tree constraints
    # ------------------------------------------------------------

    # Global count of instances per feature: feature_count[f] = number of active instances of f
    feature_count: FeatureList[IntVar] = FeatureList(
        [
            model.new_int_var(0, max_instances[f], f"cnt_{name(f)}")
            for f in cfm.features()
        ]
    )

    for feature in cfm.features():
        model.add(feature_count[feature] == sum(instance_vars[feature]))

    # Require constraints:
    # (feature_count[a] ∈ cardA) => (feature_count[b] ∈ cardB)
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

    # Exclude constraints:
    # ¬( (feature_count[a] ∈ cardA) ∧ (feature_count[b] ∈ cardB) )
    # which is equivalent to: (¬a_in) ∨ (¬b_in)
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
    # Dense feature instances (symmetry breaking)
    # ------------------------------------------------------------

    for feature in cfm.features():
        inst = instance_vars[feature]
        for i in range(len(inst) - 1):
            model.add(inst[i + 1] <= inst[i])

    # ------------------------------------------------------------
    # Dense parent assignment (children fill parents left-to-right)
    # ------------------------------------------------------------
    for child_feature in cfm.features():
        parent_feature = cfm.parents[child_feature]
        if parent_feature is None:
            continue

        for j in range(max_instances[child_feature] - 1):
            for i in range(max_instances[parent_feature]):
                model.add(
                    sum(parent_vars[child_feature][j][k] for k in range(i + 1))
                    >= sum(parent_vars[child_feature][j + 1][k] for k in range(i + 1))
                )

    return model, instance_vars, parent_vars, max_instances


@analyze("semi-structural-sat")
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

        sat_model, _instance_vars, _parent_vars, max_instances = cfm_to_cp_sat(model)
        total_instance_variables = sum(max_instances)

        build_time_us = int((time.perf_counter() - build_start) * 1_000_000)

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
