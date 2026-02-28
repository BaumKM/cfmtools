from cfmtools.core.cfm import (
    CFM,
    CardinalityInterval,
    CfmBuilder,
    SimpleCardinalityInterval,
)


def iv(lo: int, hi: int | None) -> CardinalityInterval:
    return CardinalityInterval([SimpleCardinalityInterval(lo, hi)])


def singleton(*vals: int) -> CardinalityInterval:
    return CardinalityInterval(SimpleCardinalityInterval(v, v) for v in vals)


def empty_by_cross_tree_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          └── A [0..1]

    Require constraints:
      - A ∈ {1} => A ∈ {0}
      - A ∈ {0} => Root ∈ {0}
    """
    b = CfmBuilder(["Root", "A"], root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")

    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 1))

    b.set_group_type_cardinality("Root", iv(0, 1))
    b.set_group_instance_cardinality("Root", iv(0, 1))

    b.add_require_constraint("A", singleton(1), singleton(0), "A")
    b.add_require_constraint("A", singleton(0), singleton(0), "Root")
    return b.build()


def empty_by_group_cardinality_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          └── A [0..1]

    Group cardinalities on Root:
      - type:     [1..1]
      - instance: [2..2]
    """
    b = CfmBuilder(["Root", "A"], root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")

    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 1))

    # Root requires exactly 2 child instances,
    # but A can contribute at most 1.
    b.set_group_type_cardinality("Root", iv(1, 1))
    b.set_group_instance_cardinality("Root", iv(2, 2))

    b.set_group_type_cardinality("A", iv(0, 0))
    b.set_group_instance_cardinality("A", iv(0, 0))

    return b.build()


def empty_by_feature_cardinality_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          └── A [0..0]
    """
    b = CfmBuilder(["Root", "A"], root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")

    b.set_feature_instance_cardinality("Root", iv(1, 1))

    b.set_feature_instance_cardinality("A", iv(0, 0))

    b.set_group_type_cardinality("Root", iv(1, 1))
    b.set_group_instance_cardinality("Root", iv(1, 1))

    b.set_group_type_cardinality("A", iv(0, 0))
    b.set_group_instance_cardinality("A", iv(0, 0))

    return b.build()
