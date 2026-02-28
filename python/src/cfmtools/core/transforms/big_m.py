from cfmtools.core.cfm import CFM, FeatureList
from cfmtools.core.transforms.trivial_dead_cardinalities import (
    eliminate_trivial_dead_cardinalities,
)


def _subtree_sizes(cfm: CFM) -> FeatureList[int]:
    """
    Return subtree size for each feature (including itself).
    subtree_size[f] = number of features in the subtree rooted at f.
    """
    sizes: FeatureList[int] = FeatureList([0] * cfm.n_features)

    for f in cfm.traverse_postorder():
        total = 1  # count self
        for ch in cfm.children[f]:
            total += sizes[ch]
        sizes[f] = total

    return sizes


def _cross_tree_non_convex_max(cfm: CFM) -> int:
    """
    Compute the maximum non-convex bound over all cross-tree constraints.
    """

    max_bound: int = 0
    for rc in cfm.require_constraints:
        max_bound = max(
            max_bound,
            rc.first_cardinality.non_convex_bound,
            rc.second_cardinality.non_convex_bound,
        )

    for ec in cfm.exclude_constraints:
        max_bound = max(
            max_bound,
            ec.first_cardinality.non_convex_bound,
            ec.second_cardinality.non_convex_bound,
        )
    return max_bound


def _feature_instance_bounds(cfm: CFM) -> FeatureList[int]:
    """
    Compute the feature-specific Big-M bound M_f according to:

        M_f = max(
            1,
            non_convex_bound(feature_instance(f)),
            non_convex_bound(group_instance(f)),
            non_convex_bound(group_type(f)),
            B * N_f
        )

    where:
        N_f = subtree size of f
        B   = max non-convex bound over all cross-tree constraints
    """
    cross_tree_bound = _cross_tree_non_convex_max(cfm)
    subtree_sizes = _subtree_sizes(cfm)
    feature_instance_bounds = FeatureList([0] * cfm.n_features)

    for f in cfm.features():
        feature_instance_bound = cfm.feature_instance_cardinalities[f].non_convex_bound
        group_instance_bound = cfm.group_instance_cardinalities[f].non_convex_bound
        group_type_bound = cfm.group_type_cardinalities[f].non_convex_bound

        feature_instance_bounds[f] = max(
            1,
            feature_instance_bound,
            group_instance_bound,
            group_type_bound,
            cross_tree_bound * subtree_sizes[f],
        )

    return feature_instance_bounds


def apply_big_m(
    cfm: CFM,
) -> CFM:
    """
    Apply a conservative Big-M bound to make the CFM finite while preserving all
    non-convex behavior.
    """
    feature_instance_bounds = _feature_instance_bounds(cfm)
    new_feature_instance_cardinalities = FeatureList(cfm.feature_instance_cardinalities)
    new_group_instance_cardinalities = FeatureList(cfm.group_instance_cardinalities)
    new_group_type_cardinalities = FeatureList(cfm.group_type_cardinalities)
    for f in cfm.features():
        new_feature_instance_cardinalities[f] = new_feature_instance_cardinalities[
            f
        ].bound(feature_instance_bounds[f])

    bounded_cfm = cfm.change_cardinalities(
        new_feature_instance_cardinalities,
        new_group_instance_cardinalities,
        new_group_type_cardinalities,
    )
    return eliminate_trivial_dead_cardinalities(bounded_cfm)
