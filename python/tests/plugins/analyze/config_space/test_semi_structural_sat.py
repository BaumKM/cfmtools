from dataclasses import dataclass, field
from itertools import product
from typing import Callable, TypeAlias
from cfmtools.core.cfm import CFM, FeatureList, FeatureName

from cfmtools.plugins.analyze.config_space.semi_structural_sat import cfm_to_cp_sat
from tests.data.target.memory.cfm.basic_empty import (
    empty_by_cross_tree_cfm,
    empty_by_feature_cardinality_cfm,
    empty_by_group_cardinality_cfm,
)
from tests.data.target.memory.cfm.basic_ctc import (
    exclude_simple_cfm,
    mixed_constraints_cfm,
    require_simple_cfm,
    single_config_deep_cfm,
    two_config_deep_cfm,
)
from tests.data.target.memory.cfm.basic import (
    cutoff_cfm,
    dead_branch_cfm,
    deep_cfm,
    deep_chain_cfm,
    gap_cfm,
    group_restricted_cfm,
    large_gap_cfm,
    simple_cfm,
    wide_cfm,
)
from tests.data.target.uvl.fm.valid.sandwich import sandwich_cfm
from ortools.sat.python import cp_model

from tests.data.target.uvl.fm.valid.table import table_cfm


def enumerate_configurations(
    cfm: CFM,
    model: cp_model.CpModel,
    instance_vars: FeatureList[list[cp_model.IntVar]],
    parent_vars: FeatureList[list[list[cp_model.IntVar]]],
    vars_per_feature: FeatureList[int],
) -> list[InstNode]:
    solver = cp_model.CpSolver()
    solver.parameters.enumerate_all_solutions = True

    configs: list[InstNode] = []

    class Collector(cp_model.CpSolverSolutionCallback):
        def on_solution_callback(self):
            root = decode_solution(
                cfm,
                self,
                instance_vars,
                parent_vars,
                vars_per_feature,
            )
            configs.append(root)

    status = solver.solve(model, Collector())
    assert status == cp_model.OPTIMAL

    return configs


def print_tree(cfm: CFM, node: InstNode, indent: str = ""):
    # +1 since feature instances indices are actually 1 based
    print(f"{indent}{node.feature_name}[{node.idx}]")

    node.children.sort(key=lambda n: (n.feature_name, n.idx))

    for ch in node.children:
        print_tree(cfm, ch, indent + "  ")


TupleNode: TypeAlias = tuple[
    FeatureName,
    int,
    tuple["TupleNode", ...],
]


@dataclass
class InstNode:
    feature_name: FeatureName
    idx: int
    children: list[InstNode] = field(default_factory=list["InstNode"])

    def tuple_repr(self) -> TupleNode:
        return (
            self.feature_name,
            self.idx,
            tuple(
                ch.tuple_repr()
                for ch in sorted(
                    self.children,
                    key=lambda n: (n.feature_name, n.idx),
                )
            ),
        )


def decode_solution(
    cfm: CFM,
    solver: cp_model.CpSolverSolutionCallback,
    instance_vars: FeatureList[list[cp_model.IntVar]],
    parent_vars: FeatureList[list[list[cp_model.IntVar]]],
    vars_per_feature: FeatureList[int],
) -> InstNode:
    root = cfm.root

    nodes: FeatureList[list[InstNode | None]] = FeatureList(
        [[] for _ in range(cfm.n_features)]
    )

    for feature in cfm.features():
        nodes[feature] = [None] * vars_per_feature[feature]
        for i in range(vars_per_feature[feature]):
            if solver.value(instance_vars[feature][i]) == 1:
                nodes[feature][i] = InstNode(
                    feature_name=cfm.feature_name(feature), idx=i
                )

    # Attach children to parents based on parent_vars
    for child in cfm.features():
        parent = cfm.parents[child]
        if parent is None:
            continue

        for ci in range(vars_per_feature[child]):
            child_node = nodes[child][ci]
            if child_node is None:
                continue

            # find which parent instance it's attached to
            attached_parent_idx = None
            for pj in range(vars_per_feature[parent]):
                if solver.value(parent_vars[child][ci][pj]) == 1:
                    attached_parent_idx = pj
                    break

            assert attached_parent_idx is not None

            parent_node = nodes[parent][attached_parent_idx]

            assert parent_node is not None

            parent_node.children.append(child_node)

    root_node = nodes[root][0]
    assert root_node is not None
    return root_node


def N(name: str, idx: int, *children: InstNode) -> InstNode:
    return InstNode(
        feature_name=FeatureName(name),
        idx=idx,
        children=list(children),
    )


class SandwichConfigurations:
    @staticmethod
    def cfg_bread():
        return N(
            "Sandwich",
            0,
            N("dummy_Sandwich_0", 0, N("Bread", 0)),
            N("dummy_Sandwich_1", 0),
        )

    @staticmethod
    def cfg_bread_cheese():
        return N(
            "Sandwich",
            0,
            N("dummy_Sandwich_0", 0, N("Bread", 0)),
            N("dummy_Sandwich_1", 0, N("Cheese", 0)),
        )

    @staticmethod
    def cfg_bread_mustard():
        return N(
            "Sandwich",
            0,
            N("dummy_Sandwich_0", 0, N("Bread", 0)),
            N(
                "dummy_Sandwich_1",
                0,
                N("Sauce", 0, N("Mustard", 0)),
            ),
        )

    @staticmethod
    def cfg_bread_mustard_cheese():
        return N(
            "Sandwich",
            0,
            N("dummy_Sandwich_0", 0, N("Bread", 0)),
            N(
                "dummy_Sandwich_1",
                0,
                N("Cheese", 0),
                N("Sauce", 0, N("Mustard", 0)),
            ),
        )

    @staticmethod
    def cfg_bread_ketchup_cheese():
        return N(
            "Sandwich",
            0,
            N("dummy_Sandwich_0", 0, N("Bread", 0)),
            N(
                "dummy_Sandwich_1",
                0,
                N("Cheese", 0),
                N("Sauce", 0, N("Ketchup", 0)),
            ),
        )

    @classmethod
    def configurations(cls) -> list[InstNode]:
        return [
            cls.cfg_bread(),
            cls.cfg_bread_cheese(),
            cls.cfg_bread_mustard(),
            cls.cfg_bread_mustard_cheese(),
            cls.cfg_bread_ketchup_cheese(),
        ]


def test_sandwich_semi_structural_sat():
    cfm = sandwich_cfm()

    model, instance_vars, parent_vars, vars_per_feature = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(
        cfm, model, instance_vars, parent_vars, vars_per_feature
    )
    targets = SandwichConfigurations.configurations()

    found_set = {config.tuple_repr() for config in found}
    target_set = {config.tuple_repr() for config in targets}

    missing = target_set - found_set
    extra = found_set - target_set

    if missing or extra:
        print("\n=== CONFIGURATION MISMATCH ===")

        if missing:
            print("\nMissing configurations:")
            for cfg in missing:
                print(cfg)

        if extra:
            print("\nUnexpected configurations:")
            for cfg in extra:
                print(cfg)

        print("\n==============================")

    assert not missing and not extra


class TableConfigurations:
    @staticmethod
    def cfg_table_single_set_unidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToCategorical",
                            0,
                            N("SingleSetOfCategories", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Unidirectional", 0),
                ),
            ),
        )

    @staticmethod
    def cfg_table_hierarchical_unidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToCategorical",
                            0,
                            N("HierarchicalCategories", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Unidirectional", 0),
                ),
            ),
        )

    @staticmethod
    def cfg_table_hierarchical_bidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToCategorical",
                            0,
                            N("HierarchicalCategories", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Bidirectional", 0),
                ),
            ),
        )

    @staticmethod
    def cfg_table_multiple_categories_bidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToCategorical",
                            0,
                            N("MultipleCategories", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Bidirectional", 0),
                ),
            ),
        )

    @staticmethod
    def cfg_table_multiple_categories_unidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToCategorical",
                            0,
                            N("MultipleCategories", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Unidirectional", 0),
                ),
            ),
        )

    @staticmethod
    def cfg_table_single_items_unidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToQuantitative",
                            0,
                            N("SingleCategoricalItems", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Unidirectional", 0),
                ),
            ),
        )

    @staticmethod
    def cfg_table_multiple_items_unidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToQuantitative",
                            0,
                            N("MultipleCategoricalItems", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Unidirectional", 0),
                ),
            ),
        )

    @staticmethod
    def cfg_table_multiple_items_bidirectional():
        return N(
            "Table",
            0,
            N(
                "dummy_Table_0",
                0,
                N(
                    "Information",
                    0,
                    N(
                        "DataRelationship",
                        0,
                        N(
                            "QuantitativeToQuantitative",
                            0,
                            N("MultipleCategoricalItems", 0),
                        ),
                    ),
                ),
            ),
            N(
                "dummy_Table_1",
                0,
                N(
                    "TableType",
                    0,
                    N("Bidirectional", 0),
                ),
            ),
        )

    @classmethod
    def configurations(cls):
        return [
            cls.cfg_table_single_set_unidirectional(),
            cls.cfg_table_hierarchical_unidirectional(),
            cls.cfg_table_hierarchical_bidirectional(),
            cls.cfg_table_multiple_categories_bidirectional(),
            cls.cfg_table_multiple_categories_unidirectional(),
            cls.cfg_table_single_items_unidirectional(),
            cls.cfg_table_multiple_items_unidirectional(),
            cls.cfg_table_multiple_items_bidirectional(),
        ]


def test_table_semi_structural_sat():
    cfm = table_cfm()

    model, instance_vars, parent_vars, vars_per_feature = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(
        cfm, model, instance_vars, parent_vars, vars_per_feature
    )

    targets = TableConfigurations.configurations()
    found_set = {config.tuple_repr() for config in found}
    target_set = {config.tuple_repr() for config in targets}

    missing = target_set - found_set
    extra = found_set - target_set

    if missing or extra:
        print("\n=== CONFIGURATION MISMATCH ===")

        if missing:
            print("\nMissing configurations:")
            for cfg in missing:
                print(cfg)

        if extra:
            print("\nUnexpected configurations:")
            for cfg in extra:
                print(cfg)

        print("\n==============================")

    assert not missing and not extra


class SimpleConfigurations:
    @staticmethod
    def cfg_root():
        return N("Root", 0)

    @staticmethod
    def cfg_root_a():
        return N("Root", 0, N("A", 0))

    @staticmethod
    def cfg_root_b():
        return N("Root", 0, N("B", 0))

    @staticmethod
    def cfg_root_a_b():
        return N("Root", 0, N("A", 0), N("B", 0))

    @classmethod
    def configurations(cls):
        return [
            cls.cfg_root(),
            cls.cfg_root_a(),
            cls.cfg_root_b(),
            cls.cfg_root_a_b(),
        ]


def test_simple_semi_structural_sat():
    cfm = simple_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    found_set = {config.tuple_repr() for config in found}
    target_set = {
        config.tuple_repr() for config in SimpleConfigurations.configurations()
    }

    assert found_set == target_set


def mult(feature: str, k: int):
    return [N(feature, i) for i in range(k)]


class WideConfigurations:
    @classmethod
    def configurations(cls):
        cfgs: list[InstNode] = []
        for a in range(3):
            for b in range(3):
                for c in range(3):
                    children: list[InstNode] = []
                    children += mult("A", a)
                    children += mult("B", b)
                    children += mult("C", c)
                    cfgs.append(N("Root", 0, *children))
        return cfgs


def test_wide_semi_structural_sat():
    cfm = wide_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    found_set = {config.tuple_repr() for config in found}
    target_set = {config.tuple_repr() for config in WideConfigurations.configurations()}

    assert found_set == target_set


class DeepConfigurations:
    @dataclass(frozen=True)
    class IndexState:
        counts: dict[str, int]

        @staticmethod
        def empty() -> "DeepConfigurations.IndexState":
            return DeepConfigurations.IndexState({})

        def fresh(self, label: str) -> tuple[int, "DeepConfigurations.IndexState"]:
            i = self.counts.get(label, 0)
            new_counts = dict(self.counts)
            new_counts[label] = i + 1
            return i, DeepConfigurations.IndexState(new_counts)

    @staticmethod
    def gen_nodes(
        label: str,
        mult: int,
        state: IndexState,
        child_fn: Callable[
            [DeepConfigurations.IndexState],
            list[tuple[list[InstNode], DeepConfigurations.IndexState]],
        ],
    ) -> list[tuple[list[InstNode], DeepConfigurations.IndexState]]:
        if mult == 0:
            return [([], state)]

        partials: list[tuple[list[InstNode], DeepConfigurations.IndexState]] = [
            ([], state)
        ]

        for _ in range(mult):
            new_partials: list[tuple[list[InstNode], DeepConfigurations.IndexState]] = (
                []
            )
            for nodes, st in partials:
                idx, st1 = st.fresh(label)
                for children, st2 in child_fn(st1):
                    new_partials.append((nodes + [N(label, idx, *children)], st2))
            partials = new_partials

        return partials

    @staticmethod
    def gen_b(state: IndexState):
        # generate all possible configurations below a single b instance
        results: list[tuple[list[InstNode], DeepConfigurations.IndexState]] = []
        for c_mult in (0, 1, 2):
            st = state
            cs: list[InstNode] = []
            for _ in range(c_mult):
                c_idx, st = st.fresh("C")
                cs.append(N("C", c_idx))
            results.append((cs, st))
        return results

    @staticmethod
    def gen_a(state: IndexState):
        results: list[tuple[list[InstNode], DeepConfigurations.IndexState]] = []
        for b_mult in (0, 1, 2):
            results.extend(
                DeepConfigurations.gen_nodes(
                    "B", b_mult, state, DeepConfigurations.gen_b
                )
            )
        return results

    @staticmethod
    def gen_x(state: IndexState):
        results: list[tuple[list[InstNode], DeepConfigurations.IndexState]] = []
        for y_mult in (0, 1, 2):
            st = state
            ys: list[InstNode] = []
            for _ in range(y_mult):
                y_idx, st = st.fresh("Y")
                ys.append(N("Y", y_idx))
            results.append((ys, st))
        return results

    @classmethod
    def configurations(cls) -> list[InstNode]:
        cfgs: list[InstNode] = []
        mults = (0, 1, 2)

        for a_mult, x_mult in product(mults, repeat=2):
            base = DeepConfigurations.IndexState.empty()

            a_variants = cls.gen_nodes("A", a_mult, base, cls.gen_a)
            x_variants = cls.gen_nodes("X", x_mult, base, cls.gen_x)

            for (as_, _), (xs, _) in product(a_variants, x_variants):
                cfgs.append(N("Root", 0, *(as_ + xs)))

        return cfgs


def test_deep_semi_structural_sat():
    cfm = deep_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)
    target_set = {config.tuple_repr() for config in DeepConfigurations.configurations()}
    found_set = {config.tuple_repr() for config in found}

    assert found_set == target_set


class GapConfigurations:
    @staticmethod
    def configurations():
        return [
            N("Root", 0),
            N("Root", 0, N("A", 0), N("A", 1)),
            N("Root", 0, N("B", 0), N("B", 1)),
        ]


def test_gap_semi_structural_sat():
    cfm = gap_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    assert {config.tuple_repr() for config in found} == {
        config.tuple_repr() for config in GapConfigurations.configurations()
    }


class LargeGapConfigurations:
    @classmethod
    def configurations(cls):
        allowed = [0, 5, 10, 1000]
        cfgs: list[InstNode] = []

        for a in allowed:
            for b in allowed:
                if a == b == 0:
                    cfgs.append(N("Root", 0))
                elif a == 0:
                    cfgs.append(N("Root", 0, *[N("B", i) for i in range(b)]))
                elif b == 0:
                    cfgs.append(N("Root", 0, *[N("A", i) for i in range(a)]))
                else:
                    cfgs.append(
                        N(
                            "Root",
                            0,
                            *[N("A", i) for i in range(a)],
                            *[N("B", i) for i in range(b)],
                        )
                    )
        return cfgs


def test_large_gap_semi_structural_sat():
    cfm = large_gap_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    assert {config.tuple_repr() for config in found} == {
        config.tuple_repr() for config in LargeGapConfigurations.configurations()
    }


class CutoffConfigurations:
    @staticmethod
    def configurations():
        return [
            N("Root", 0),
            N("Root", 0, N("A", 0)),
        ]


def test_cutoff_semi_structural_sat():
    cfm = cutoff_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    assert {config.tuple_repr() for config in found} == {
        config.tuple_repr() for config in CutoffConfigurations.configurations()
    }


class DeepChainConfigurations:
    @classmethod
    def configurations(cls):
        cfgs: list[InstNode] = []
        for mask in range(8):
            leaves: list[InstNode] = []
            if mask & 1:
                leaves.append(N("X", 0))
            if mask & 2:
                leaves.append(N("Y", 0))
            if mask & 4:
                leaves.append(N("Z", 0))

            cfgs.append(
                N(
                    "Root",
                    0,
                    N(
                        "A",
                        0,
                        N(
                            "B",
                            0,
                            N(
                                "C",
                                0,
                                N(
                                    "D",
                                    0,
                                    N("LeafRoot", 0, *leaves),
                                ),
                            ),
                        ),
                    ),
                )
            )
        return cfgs


def test_deep_chain_semi_structural_sat():
    cfm = deep_chain_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    assert {config.tuple_repr() for config in found} == {
        config.tuple_repr() for config in DeepChainConfigurations.configurations()
    }


class GroupRestrictedConfigurations:
    @classmethod
    def configurations(cls):
        cfgs = [N("Root", 0, N("A", 0))]

        for k in (1, 2):
            cfgs.append(N("Root", 0, N("A", 0, *[N("X", i) for i in range(k)])))
            cfgs.append(N("Root", 0, N("A", 0, *[N("Y", i) for i in range(k)])))

        return cfgs


def test_group_restricted_semi_structural_sat():
    cfm = group_restricted_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    assert {config.tuple_repr() for config in found} == {
        config.tuple_repr() for config in GroupRestrictedConfigurations.configurations()
    }


class DeadBranchConfigurations:
    @staticmethod
    def configurations():
        return [N("Root", 0, N("B", 0))]


def test_dead_branch_semi_structural_sat():
    cfm = dead_branch_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    assert {config.tuple_repr() for config in found} == {
        config.tuple_repr() for config in DeadBranchConfigurations.configurations()
    }


class RequireSimpleConfigurations:
    @staticmethod
    def configurations():
        cfgs: list[InstNode] = []
        for a in (0, 1, 2):
            for b in (0, 1, 2):
                if a == 2 and b != 1:
                    continue
                children: list[InstNode] = []
                children += [N("A", i) for i in range(a)]
                children += [N("B", i) for i in range(b)]
                cfgs.append(N("Root", 0, *children))
        return cfgs


def test_require_simple_semi_structural_sat():
    cfm = require_simple_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)

    found = enumerate_configurations(cfm, model, iv, pv, vpf)
    found_set = {config.tuple_repr() for config in found}
    target_set = {
        config.tuple_repr() for config in RequireSimpleConfigurations.configurations()
    }

    assert found_set == target_set


class ExcludeSimpleConfigurations:
    @staticmethod
    def configurations():
        cfgs: list[InstNode] = []
        for a in (0, 1, 2):
            for b in (0, 1, 2):
                if a == 2 and b == 2:
                    continue
                children: list[InstNode] = []
                children += [N("A", i) for i in range(a)]
                children += [N("B", i) for i in range(b)]
                cfgs.append(N("Root", 0, *children))
        return cfgs


def test_exclude_simple_semi_structural_sat():
    cfm = exclude_simple_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)

    found = enumerate_configurations(cfm, model, iv, pv, vpf)
    found_set = {config.tuple_repr() for config in found}
    target_set = {
        config.tuple_repr() for config in ExcludeSimpleConfigurations.configurations()
    }

    assert found_set == target_set


class MixedConstraintsConfigurations:
    @staticmethod
    def configurations():
        cfgs: list[InstNode] = []
        for a in (0, 1, 2, 3):
            for b in (0, 1, 2, 3):
                if a in (1, 3) and b != 2:
                    continue
                if a == 3 and b == 3:
                    continue
                children: list[InstNode] = []
                children += [N("A", i) for i in range(a)]
                children += [N("B", i) for i in range(b)]
                cfgs.append(N("Root", 0, *children))
        return cfgs


def test_mixed_constraints_semi_structural_sat():
    cfm = mixed_constraints_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)

    found = enumerate_configurations(cfm, model, iv, pv, vpf)
    found_set = {config.tuple_repr() for config in found}
    target_set = {
        config.tuple_repr()
        for config in MixedConstraintsConfigurations.configurations()
    }

    assert found_set == target_set


class SingleConfigDeepConfigurations:
    @staticmethod
    def configurations():
        return [
            N(
                "Root",
                0,
                N(
                    "A",
                    0,
                    N("Y", 0),
                    N("Z", 0),
                    N("Z", 1),
                ),
            )
        ]


def test_single_config_deep_semi_structural_sat():
    cfm = single_config_deep_cfm()
    model, iv, pv, vpf = cfm_to_cp_sat(cfm)

    found = enumerate_configurations(cfm, model, iv, pv, vpf)
    found_set = {config.tuple_repr() for config in found}
    target_set = {
        config.tuple_repr()
        for config in SingleConfigDeepConfigurations.configurations()
    }
    assert found_set == target_set


class TwoConfigDeepConfigurations:
    @staticmethod
    def cfg_a5_b0_c5():
        # A = 5, B = 0, C = 5
        return N(
            "Root",
            0,
            *[N("A", i, N("C", i)) for i in range(5)],
        )

    @staticmethod
    def cfg_a5_b5_c5():
        # A = 5, B = 5, C = 5
        return N(
            "Root",
            0,
            *[N("A", i, N("B", i), N("C", i)) for i in range(5)],
        )

    @classmethod
    def configurations(cls):
        return [
            cls.cfg_a5_b0_c5(),
            cls.cfg_a5_b5_c5(),
        ]


def test_two_config_deep_cfm():
    cfm = two_config_deep_cfm()

    model, iv, pv, vpf = cfm_to_cp_sat(cfm)
    found = enumerate_configurations(cfm, model, iv, pv, vpf)

    found_set = {config.tuple_repr() for config in found}
    target_set = {
        config.tuple_repr() for config in TwoConfigDeepConfigurations.configurations()
    }
    assert found_set == target_set


def solve_status_only(
    model: cp_model.CpModel,
) -> cp_model.CpSolverStatus:
    solver = cp_model.CpSolver()
    return solver.solve(model)


def test_empty_by_cross_tree_semi_structural_sat():
    cfm = empty_by_cross_tree_cfm()

    model, _, _, _ = cfm_to_cp_sat(cfm)
    status = solve_status_only(model)

    assert status == cp_model.INFEASIBLE


def test_empty_by_group_cardinality_semi_structural_sat():
    cfm = empty_by_group_cardinality_cfm()

    model, _, _, _ = cfm_to_cp_sat(cfm)
    status = solve_status_only(model)

    assert status == cp_model.INFEASIBLE


def test_empty_by_feature_cardinality_semi_structural_sat():
    cfm = empty_by_feature_cardinality_cfm()

    model, _, _, _ = cfm_to_cp_sat(cfm)
    status = solve_status_only(model)

    assert status == cp_model.INFEASIBLE
