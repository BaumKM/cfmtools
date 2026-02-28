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


def require_simple_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
        ├── A [0..2]
        └── B [0..2]

    Require constraints:
    - A ∈ {2} => B ∈ {1}
    """
    b = CfmBuilder(["Root", "A", "B"], root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 2))
    b.set_feature_instance_cardinality("B", iv(0, 2))

    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(0, 4))

    # A = 2  =>  B = 1
    b.add_require_constraint(
        "A",
        singleton(2),
        singleton(1),
        "B",
    )
    return b.build()


def exclude_simple_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
        ├── A [0..2]
        └── B [0..2]

    Exclude constraints:
    - A ∈ {2} x B ∈ {2}
    """

    b = CfmBuilder(["Root", "A", "B"], root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 2))
    b.set_feature_instance_cardinality("B", iv(0, 2))

    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(0, 4))

    # forbid (A=2, B=2)
    b.add_exclude_constraint(
        "A",
        singleton(2),
        singleton(2),
        "B",
    )

    return b.build()


def mixed_constraints_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          ├── A [0..3]
          └── B [0..3]

        Require constraints:
        - A ∈ {1,3} => B ∈ {2}

        Exclude constraints:
        - A ∈ {3} x B ∈ {3}
    """
    b = CfmBuilder(["Root", "A", "B"], root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 3))
    b.set_feature_instance_cardinality("B", iv(0, 3))

    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(0, 6))

    # A ∈ {1,3} => B = 2
    b.add_require_constraint(
        "A",
        singleton(1, 3),
        singleton(2),
        "B",
    )

    # forbid (A=3, B=3)
    b.add_exclude_constraint(
        "A",
        singleton(3),
        singleton(3),
        "B",
    )

    return b.build()


def single_config_deep_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
        └── A [1..1]
                ├── X [0,2]
                ├── Y [0..1]
                └── Z [0,2]

    Require constraints:
    - X ∈ {2} => Y ∈ {1}
    - Y ∈ {1} => Z ∈ {2}

    Exclude constraints:
    - X ∈ {2} x Z ∈ {2}
    - Y ∈ {0} x Z ∈ {2}
    """
    b = CfmBuilder(["Root", "A", "X", "Y", "Z"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("X", "A")
    b.set_parent("Y", "A")
    b.set_parent("Z", "A")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(1, 1))
    b.set_feature_instance_cardinality("X", singleton(0, 2))
    b.set_feature_instance_cardinality("Y", iv(0, 1))
    b.set_feature_instance_cardinality("Z", singleton(0, 2))

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(1, 1))
    b.set_group_instance_cardinality("Root", iv(1, 1))

    b.set_group_type_cardinality("A", iv(1, 3))
    b.set_group_instance_cardinality("A", iv(1, 3))

    for f in ["X", "Y", "Z"]:
        b.set_group_type_cardinality(f, iv(0, 0))
        b.set_group_instance_cardinality(f, iv(0, 0))

    # Constraints
    # X = 2 => Y = 1
    b.add_require_constraint("X", singleton(2), singleton(1), "Y")

    # Y = 1 => Z = 2
    b.add_require_constraint("Y", singleton(1), singleton(2), "Z")

    # X = 2 x Z = 2
    b.add_exclude_constraint("X", singleton(2), singleton(2), "Z")

    # Y = 0 x Z = 2
    b.add_exclude_constraint("Y", singleton(0), singleton(2), "Z")

    return b.build()


def two_config_deep_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          └── A [0..5]
                ├── B [0..1]
                └── C [0..1]

    Require constraints:
      - Root ∈ {1} => B ∈ {0,5}
      - B ∈ {5} => C ∈ {5}
      - B ∈ {0} => C ∈ {5}
    """

    b = CfmBuilder(["Root", "A", "B", "C"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "A")
    b.set_parent("C", "A")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 5))
    b.set_feature_instance_cardinality("B", iv(0, 1))
    b.set_feature_instance_cardinality("C", iv(0, 1))

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(0, 1))
    b.set_group_instance_cardinality("Root", iv(0, 5))

    b.set_group_type_cardinality("A", iv(0, 2))
    b.set_group_instance_cardinality("A", iv(0, 2))

    for f in ["B", "C"]:
        b.set_group_type_cardinality(f, iv(0, 0))
        b.set_group_instance_cardinality(f, iv(0, 0))

    # Cross-tree constraints
    b.add_require_constraint("Root", singleton(1), singleton(0, 5), "B")
    b.add_require_constraint("B", singleton(5), singleton(5), "C")
    b.add_require_constraint("B", singleton(0), singleton(5), "C")

    return b.build()
