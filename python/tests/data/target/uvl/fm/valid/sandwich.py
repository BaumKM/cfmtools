from cfmtools.core.cfm import CFM, CfmBuilder
from tests.data.target.common_path import TEST_MODEL_PATH
from tests.data.target.fm_helpers import ONE, FMGroupType, configure_feature

SANDWICH_PATH = TEST_MODEL_PATH / "uvl/fm/valid/sandwich.uvl"


def sandwich_cfm() -> CFM:
    """
    Reference CFM for sandwich.uvl
    """

    feature_names = [
        "Sandwich",
        "dummy_Sandwich_0",
        "dummy_Sandwich_1",
        "Bread",
        "Sauce",
        "Ketchup",
        "Mustard",
        "Cheese",
    ]

    b = CfmBuilder(feature_names=feature_names, root="Sandwich")

    b.set_parent("Sandwich", None)

    # Sandwich → dummies
    b.set_parent("dummy_Sandwich_0", "Sandwich")
    b.set_parent("dummy_Sandwich_1", "Sandwich")

    # dummy_Sandwich_0 → Bread
    b.set_parent("Bread", "dummy_Sandwich_0")

    # dummy_Sandwich_1 → Sauce, Cheese
    b.set_parent("Sauce", "dummy_Sandwich_1")
    b.set_parent("Cheese", "dummy_Sandwich_1")

    # Sauce → Ketchup, Mustard
    b.set_parent("Ketchup", "Sauce")
    b.set_parent("Mustard", "Sauce")

    # ---------------- Sandwich ----------------
    # mandatory group, root
    configure_feature(
        b,
        "Sandwich",
        parent_group=None,
        own_group=FMGroupType.MANDATORY,
        n_children=2,
    )

    # ---------------- dummy_Sandwich_0 ----------------
    # Represents the mandatory {Bread} group
    configure_feature(
        b,
        "dummy_Sandwich_0",
        parent_group=FMGroupType.MANDATORY,
        own_group=FMGroupType.MANDATORY,
        n_children=1,
    )

    # ---------------- dummy_Sandwich_1 ----------------
    # Represents the optional {Sauce, Cheese} group
    configure_feature(
        b,
        "dummy_Sandwich_1",
        parent_group=FMGroupType.MANDATORY,
        own_group=FMGroupType.OPTIONAL,
        n_children=2,
    )

    # ---------------- Bread ----------------
    # Mandatory, leaf
    configure_feature(
        b,
        "Bread",
        parent_group=FMGroupType.MANDATORY,
        own_group=None,
        n_children=0,
    )

    # ---------------- Sauce ----------------
    # Optional, alternative group
    configure_feature(
        b,
        "Sauce",
        parent_group=FMGroupType.OPTIONAL,
        own_group=FMGroupType.ALTERNATIVE,
        n_children=2,
    )

    # ---------------- Cheese ----------------
    # Optional, leaf
    configure_feature(
        b,
        "Cheese",
        parent_group=FMGroupType.OPTIONAL,
        own_group=None,
        n_children=0,
    )

    # ---------------- Ketchup ----------------
    # Alternative, leaf
    configure_feature(
        b,
        "Ketchup",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- Mustard ----------------
    # Alternative, leaf
    configure_feature(
        b,
        "Mustard",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- Constraints ----------------

    # Ketchup => Cheese
    b.add_require_constraint("Ketchup", ONE, ONE, "Cheese")

    return b.build()
