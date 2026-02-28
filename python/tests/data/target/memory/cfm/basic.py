from cfmtools.core.cfm import (
    CFM,
    CardinalityInterval,
    CfmBuilder,
    SimpleCardinalityInterval,
)


def iv(lo: int, hi: int | None) -> CardinalityInterval:
    return CardinalityInterval([SimpleCardinalityInterval(lo, hi)])


def simple_cfm() -> CFM:
    """
    Root
      ├── A [0..1]
      └── B [0..1]
    """
    b = CfmBuilder(["Root", "A", "B"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 1))
    b.set_feature_instance_cardinality("B", iv(0, 1))

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(0, 2))

    return b.build()


def wide_cfm() -> CFM:
    """
    Root
      ├── A [0..2]
      ├── B [0..2]
      └── C [0..2]
    """

    b = CfmBuilder(["Root", "A", "B", "C"], root="Root")

    # Structure
    b.set_parent("Root", None)
    for f in ["A", "B", "C"]:
        b.set_parent(f, "Root")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    for f in ["A", "B", "C"]:
        b.set_feature_instance_cardinality(f, iv(0, 2))

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(0, 3))
    b.set_group_instance_cardinality("Root", iv(0, 6))

    return b.build()


def deep_cfm() -> CFM:
    """
    Root [1..1]
      ├── A [0..2]
      │    └── B [0..2]
      │         └── C [0..2]
      └── X [0..2]
           └── Y [0..2]
    """

    b = CfmBuilder(["Root", "A", "B", "C", "X", "Y"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("X", "Root")
    b.set_parent("B", "A")
    b.set_parent("C", "B")
    b.set_parent("Y", "X")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    for f in ["A", "B", "C", "X", "Y"]:
        b.set_feature_instance_cardinality(f, iv(0, 2))

    # Group cardinalities
    for f in ["Root", "A", "B", "X"]:
        b.set_group_type_cardinality(f, iv(0, 2))
        b.set_group_instance_cardinality(f, iv(0, 4))

    for f in ["C", "Y"]:
        b.set_group_type_cardinality(f, iv(0, 0))
        b.set_group_instance_cardinality(f, iv(0, 0))

    return b.build()


def gap_cfm() -> CFM:
    """
     Root [1..1]
       ├── A [0 or 2]
       └── B [0 or 2]

    Group cardinalities on Root:
      - type:     [0..2]
      - instance: [0..2]
    """

    b = CfmBuilder(["Root", "A", "B"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))

    gap_0_or_2 = CardinalityInterval(
        [
            SimpleCardinalityInterval(0, 0),
            SimpleCardinalityInterval(2, 2),
        ]
    )
    b.set_feature_instance_cardinality("A", gap_0_or_2)
    b.set_feature_instance_cardinality("B", gap_0_or_2)

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(0, 2))

    return b.build()


def large_gap_cfm() -> CFM:
    """
     Root [1..1]
       ├── A [0, 5, 10, 1000]
       └── B [0, 5, 10, 1000]

    Group cardinalities on Root:
      - type:     [0..2]
      - instance: [0..2000]
    """

    b = CfmBuilder(["Root", "A", "B"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))

    big_gap = CardinalityInterval(
        [
            SimpleCardinalityInterval(0, 0),
            SimpleCardinalityInterval(5, 5),
            SimpleCardinalityInterval(10, 10),
            SimpleCardinalityInterval(1000, 1000),
        ]
    )

    b.set_feature_instance_cardinality("A", big_gap)
    b.set_feature_instance_cardinality("B", big_gap)

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(0, 2000))

    return b.build()


def cutoff_cfm() -> CFM:
    """
        Root [1..1]
          ├── A [0..1]
          └── X [∅]        (unreachable)
                ├── Y [0..25]
                └── Z [0..25]

    Group cardinalities:
      - Root:
          * type:     [0..2]
          * instance: [0..2]
      - X:
          * type:     [0..50]
          * instance: [0..50]
    """

    b = CfmBuilder(["Root", "A", "X", "Y", "Z"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("X", "Root")
    b.set_parent("Y", "X")
    b.set_parent("Z", "X")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 1))

    empty = CardinalityInterval([])  # normalized to [0,0] internally
    b.set_feature_instance_cardinality("X", empty)

    b.set_feature_instance_cardinality("Y", iv(0, 25))
    b.set_feature_instance_cardinality("Z", iv(0, 25))

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(0, 2))

    b.set_group_type_cardinality("X", iv(0, 50))
    b.set_group_instance_cardinality("X", iv(0, 50))

    return b.build()


def deep_chain_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          └── A [1..1]
                └── B [1..1]
                      └── C [1..1]
                            └── D [1..1]
                                  └── LeafRoot [1..1]
                                        ├── X [0..1]
                                        ├── Y [0..1]
                                        └── Z [0..1]

    Group cardinalities:
      - For all chain nodes (Root, A, B, C, D, LeafRoot):
          * type:     [0..3]
          * instance: [0..3]
      - Leaves (X, Y, Z) have no children.
    """

    b = CfmBuilder(
        ["Root", "A", "B", "C", "D", "LeafRoot", "X", "Y", "Z"],
        root="Root",
    )

    # Structure
    chain = ["Root", "A", "B", "C", "D", "LeafRoot"]
    for i, f in enumerate(chain):
        b.set_parent(f, None if i == 0 else chain[i - 1])

    for f in ["X", "Y", "Z"]:
        b.set_parent(f, "LeafRoot")

    # Feature instance cardinalities
    for f in chain:
        b.set_feature_instance_cardinality(f, iv(1, 1))

    for f in ["X", "Y", "Z"]:
        b.set_feature_instance_cardinality(f, iv(0, 1))

    # Group cardinalities
    for f in chain:
        b.set_group_type_cardinality(f, iv(0, 3))
        b.set_group_instance_cardinality(f, iv(0, 3))

    for f in ["X", "Y", "Z"]:
        b.set_group_type_cardinality(f, iv(0, 0))
        b.set_group_instance_cardinality(f, iv(0, 0))

    return b.build()


def group_restricted_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          └── A [1..1]
                ├── X [0..10]
                └── Y [0..10]

    Group cardinalities on A:
      - type:     [0..1]
      - instance: [0..2]

    """
    b = CfmBuilder(["Root", "A", "X", "Y"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("X", "A")
    b.set_parent("Y", "A")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(1, 1))
    b.set_feature_instance_cardinality("X", iv(0, 10))
    b.set_feature_instance_cardinality("Y", iv(0, 10))

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(1, 1))
    b.set_group_instance_cardinality("Root", iv(1, 1))

    b.set_group_type_cardinality("A", iv(0, 1))
    b.set_group_instance_cardinality("A", iv(0, 2))

    return b.build()


def dead_branch_cfm() -> CFM:
    """
    Structure:

        Root [1..1]
          ├── A [0..1]          (optional)
          │     └── Dead [1..1]
          │           ├── X [0..1]
          │           └── Y [0..1]
          │
          │   Dead group cardinalities:
          │     - type:     [3..3]
          │     - instance: [3..3]
          │
          │   (Impossible to satisfy because Dead has only two children: X and Y.)
          │
          └── B [1..1]         (mandatory)

    """

    b = CfmBuilder(["Root", "A", "Dead", "X", "Y", "B"], root="Root")

    # Structure
    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")
    b.set_parent("Dead", "A")
    b.set_parent("X", "Dead")
    b.set_parent("Y", "Dead")

    # Feature instance cardinalities
    b.set_feature_instance_cardinality("Root", iv(1, 1))
    b.set_feature_instance_cardinality("A", iv(0, 1))
    b.set_feature_instance_cardinality("Dead", iv(1, 1))
    b.set_feature_instance_cardinality("X", iv(0, 1))
    b.set_feature_instance_cardinality("Y", iv(0, 1))
    b.set_feature_instance_cardinality("B", iv(1, 1))

    # Group cardinalities
    b.set_group_type_cardinality("Root", iv(0, 2))
    b.set_group_instance_cardinality("Root", iv(1, 2))

    b.set_group_type_cardinality("A", iv(1, 1))
    b.set_group_instance_cardinality("A", iv(1, 1))

    b.set_group_type_cardinality("Dead", iv(3, 3))
    b.set_group_instance_cardinality("Dead", iv(3, 3))

    return b.build()
