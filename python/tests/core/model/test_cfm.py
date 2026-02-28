import pytest

from cfmtools.core.cfm import (
    CFM,
    CfmBuilder,
    CardinalityInterval,
    Feature,
    FeatureList,
    FeatureName,
    FeatureTuple,
    SimpleCardinalityInterval,
)


def exactly(value: int) -> CardinalityInterval:
    return CardinalityInterval([SimpleCardinalityInterval(value, value)])


def test_build_minimal_tree():
    """
    root
     |
    child
    """

    builder = CfmBuilder(
        feature_names=["root", "child"],
        root="root",
    )

    # Tree
    builder.set_parent("child", "root")

    # Root must be exactly (1,1)
    builder.set_feature_instance_cardinality("root", exactly(1))

    # Leaf group cardinalities must be exactly (0,0)
    builder.set_group_instance_cardinality("child", exactly(0))
    builder.set_group_type_cardinality("child", exactly(0))

    cfm = builder.build()

    assert cfm.n_features == 2
    assert cfm.feature_name(cfm.root) == "root"

    child_id = cfm.feature(FeatureName("child"))
    assert cfm.parents[child_id] == cfm.root
    assert cfm.is_leaf(child_id)


def test_duplicate_feature_names_rejected():
    with pytest.raises(ValueError):
        CfmBuilder(
            feature_names=["a", "a"],
            root="a",
        )


def test_missing_root_rejected():
    with pytest.raises(ValueError):
        CfmBuilder(
            feature_names=["a", "b"],
            root="c",
        )


def test_invalid_root_cardinality_detected():
    builder = CfmBuilder(
        feature_names=["root"],
        root="root",
    )

    # Wrong: root cardinality must be exactly (1,1)
    builder.set_feature_instance_cardinality("root", exactly(2))

    with pytest.raises(AssertionError):
        builder.build()


def test_leaf_invalid_group_instance_cardinality():
    """
    Leaf must have group instance cardinality exactly (0,0).
    """

    builder = CfmBuilder(
        feature_names=["root", "leaf"],
        root="root",
    )

    builder.set_parent("leaf", "root")

    builder.set_feature_instance_cardinality("root", exactly(1))

    # invalid for leaf
    builder.set_group_instance_cardinality("leaf", exactly(1))
    builder.set_group_type_cardinality("leaf", exactly(0))

    with pytest.raises(AssertionError):
        builder.build()


def test_leaf_invalid_group_type_cardinality():
    """
    Leaf must have group type cardinality exactly (0,0).
    """

    builder = CfmBuilder(
        feature_names=["root", "leaf"],
        root="root",
    )

    builder.set_parent("leaf", "root")

    builder.set_feature_instance_cardinality("root", exactly(1))

    builder.set_group_instance_cardinality("leaf", exactly(0))
    # invalid for leaf
    builder.set_group_type_cardinality("leaf", exactly(1))

    with pytest.raises(AssertionError):
        builder.build()


def test_root_with_parent():
    """
    Root is not allowed to have a parent.
    """

    builder = CfmBuilder(
        feature_names=["root"],
        root="root",
    )

    builder.set_parent("root", "root")

    builder.set_feature_instance_cardinality("root", exactly(1))
    builder.set_group_instance_cardinality("root", exactly(0))
    builder.set_group_type_cardinality("root", exactly(0))

    with pytest.raises(AssertionError):
        builder.build()


def test_non_root_missing_parent_rejected():
    """
    Every non-root feature must have exactly one parent.
    """

    builder = CfmBuilder(
        feature_names=["root", "orphan"],
        root="root",
    )

    # orphan has no parent set

    builder.set_feature_instance_cardinality("root", exactly(1))
    builder.set_group_instance_cardinality("orphan", exactly(0))
    builder.set_group_type_cardinality("orphan", exactly(0))

    with pytest.raises(AssertionError):
        builder.build()


def test_cycle_in_tree_rejected():
    """
    Cycles must be rejected.
    """

    builder = CfmBuilder(
        feature_names=["a", "b", "c"],
        root="a",
    )

    # Create a cycle: b <- c <- b
    builder.set_parent("c", "b")
    builder.set_parent("b", "c")

    builder.set_feature_instance_cardinality("a", exactly(1))
    builder.set_group_instance_cardinality("b", exactly(0))
    builder.set_group_type_cardinality("b", exactly(0))

    with pytest.raises(AssertionError):
        builder.build()


def test_unreachable_feature_rejected():
    """
    All features must be reachable from the root.
    """

    builder = CfmBuilder(
        feature_names=["root", "child", "unreachable"],
        root="root",
    )

    builder.set_parent("child", "root")
    builder.set_parent("unreachable", "unreachable")

    builder.set_feature_instance_cardinality("root", exactly(1))

    builder.set_group_instance_cardinality("child", exactly(0))
    builder.set_group_type_cardinality("child", exactly(0))

    builder.set_group_instance_cardinality("unreachable", exactly(0))
    builder.set_group_type_cardinality("unreachable", exactly(0))

    with pytest.raises(AssertionError):
        builder.build()


def test_cfm_rejects_inconsistent_parent_child_relation():
    """
    parents says: b is under root
    children says: a contains b
    """

    root = Feature(0)
    a = Feature(1)
    b = Feature(2)

    with pytest.raises(AssertionError):
        CFM(
            feature_names=FeatureTuple(
                [
                    FeatureName("root"),
                    FeatureName("a"),
                    FeatureName("b"),
                ]
            ),
            root=root,
            # parents: root -> a, root -> b
            parents=FeatureTuple(
                [
                    None,  # root
                    root,  # a
                    root,  # b
                ]
            ),
            # children says: a -> b
            children=FeatureTuple(
                [
                    (a,),  # root -> a   (b missing here)
                    (b,),  # a -> b      wrong parent
                    (),  # b
                ]
            ),
            # cardinalities (minimal valid)
            feature_instance_cardinalities=FeatureTuple(
                [
                    exactly(1),  # root
                    exactly(0),
                    exactly(0),
                ]
            ),
            group_instance_cardinalities=FeatureTuple(
                [
                    exactly(0),
                    exactly(0),
                    exactly(0),
                ]
            ),
            group_type_cardinalities=FeatureTuple(
                [
                    exactly(0),
                    exactly(0),
                    exactly(0),
                ]
            ),
            require_constraints=[],
            exclude_constraints=[],
        )


def test_cfm_rejects_parent_index_out_of_bounds():
    """
    leaf.parent = FeatureId(99)
    """

    root = Feature(0)

    with pytest.raises(AssertionError):
        CFM(
            feature_names=FeatureTuple([FeatureName("root"), FeatureName("leaf")]),
            root=root,
            # parent index out of bounds
            parents=FeatureTuple(
                [
                    None,
                    Feature(99),
                ]
            ),
            children=FeatureTuple(
                [
                    (),  # root
                    (),  # leaf
                ]
            ),
            feature_instance_cardinalities=FeatureTuple(
                [
                    exactly(1),
                    exactly(0),
                ]
            ),
            group_instance_cardinalities=FeatureTuple(
                [
                    exactly(0),
                    exactly(0),
                ]
            ),
            group_type_cardinalities=FeatureTuple(
                [
                    exactly(0),
                    exactly(0),
                ]
            ),
            require_constraints=[],
            exclude_constraints=[],
        )


def test_valid_three_level_tree():
    """
    root
     |
    mid
     |
    leaf
    """

    builder = CfmBuilder(
        feature_names=["root", "mid", "leaf"],
        root="root",
    )

    builder.set_parent("mid", "root")
    builder.set_parent("leaf", "mid")

    builder.set_feature_instance_cardinality("root", exactly(1))

    # leaf constraints
    builder.set_group_instance_cardinality("leaf", exactly(0))
    builder.set_group_type_cardinality("leaf", exactly(0))

    cfm = builder.build()

    assert cfm.n_features == 3
    assert cfm.is_leaf(cfm.feature(FeatureName("leaf")))
    assert not cfm.is_leaf(cfm.feature(FeatureName("mid")))


def test_cfm_basic_properties():
    """
    root
     |
     a
     |
     b
    """

    builder = CfmBuilder(
        feature_names=["root", "a", "b"],
        root="root",
    )

    builder.set_parent("a", "root")
    builder.set_parent("b", "root")

    builder.set_feature_instance_cardinality("root", exactly(1))

    # leaf constraints
    builder.set_group_instance_cardinality("a", exactly(0))
    builder.set_group_type_cardinality("a", exactly(0))
    builder.set_group_instance_cardinality("b", exactly(0))
    builder.set_group_type_cardinality("b", exactly(0))

    cfm = builder.build()

    # --- structural properties ---
    assert cfm.n_features == 3

    root_id = cfm.root
    a_id = cfm.feature(FeatureName("a"))
    b_id = cfm.feature(FeatureName("b"))

    # children / parents
    assert set(cfm.children[root_id]) == {a_id, b_id}
    assert cfm.parents[a_id] == root_id
    assert cfm.parents[b_id] == root_id

    # leaf detection
    assert cfm.is_leaf(a_id)
    assert cfm.is_leaf(b_id)
    assert not cfm.is_leaf(root_id)

    # feature ids enumeration
    assert set(cfm.features()) == {root_id, a_id, b_id}

    # name <-> id mapping
    for f_id in cfm.features():
        name = cfm.feature_name(f_id)
        assert cfm.feature(name) == f_id


def test_change_cardinalities_updates_values_and_preserves_structure():
    """
    root
     |
    leaf
    """

    # --- build base model ---
    builder = CfmBuilder(
        feature_names=["root", "leaf"],
        root="root",
    )
    builder.set_parent("leaf", "root")

    builder.set_feature_instance_cardinality("root", exactly(1))
    builder.set_group_instance_cardinality("leaf", exactly(0))
    builder.set_group_type_cardinality("leaf", exactly(0))

    # --- add cross-tree constraints ---
    req_card = exactly(1)
    excl_card = exactly(0)

    builder.add_require_constraint(
        first_feature="root",
        first_cardinality=req_card,
        second_cardinality=req_card,
        second_feature="leaf",
    )

    builder.add_exclude_constraint(
        first_feature="leaf",
        first_cardinality=excl_card,
        second_cardinality=excl_card,
        second_feature="root",
    )

    base = builder.build()

    leaf_id = base.feature(FeatureName("leaf"))

    # --- prepare new cardinalities ---
    new_feature = FeatureList(
        [
            exactly(1),  # root unchanged
            exactly(2),  # leaf changed
        ]
    )

    new_group_instance = FeatureList(
        [
            exactly(0),
            exactly(0),
        ]
    )

    new_group_type = FeatureList(
        [
            exactly(0),
            exactly(0),
        ]
    )

    # --- apply change ---
    updated = base.change_cardinalities(
        new_feature,
        new_group_instance,
        new_group_type,
    )

    # --- structure must be preserved ---
    assert updated.feature_names == base.feature_names
    assert updated.parents == base.parents
    assert updated.children == base.children
    assert updated.root == base.root

    # --- cross-tree constraints must be preserved ---
    assert updated.require_constraints == base.require_constraints
    assert updated.exclude_constraints == base.exclude_constraints

    # --- cardinalities must be updated ---
    assert updated.feature_instance_cardinalities[leaf_id].contains(2)
    assert updated.feature_instance_cardinalities[leaf_id].size == 1
