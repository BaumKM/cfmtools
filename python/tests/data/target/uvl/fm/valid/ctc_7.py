from cfmtools.core.cfm import CFM, CfmBuilder
from tests.data.target.common_path import TEST_MODEL_PATH
from tests.data.target.fm_helpers import ONE, ZERO, FMGroupType, configure_feature

CTC_7_PATH = TEST_MODEL_PATH / "uvl/fm/valid/ctc_7.uvl"


def ctc_7_cfm() -> CFM:
    feature_names = [
        "Root",
        # Implication group
        "A1",
        "B1",
        "A2",
        "B2",
        "A3",
        "B3",
        "A4",
        "B4",
        # Equivalence group
        "C1",
        "D1",
        "C2",
        "D2",
        "C3",
        "D3",
        "C4",
        "D4",
        # Excludes group
        "E1",
        "F1",
        "E2",
        "F2",
        "E3",
        "F3",
        "E4",
        "F4",
    ]

    b = CfmBuilder(feature_names=feature_names, root="Root")

    # ------------------------------------------------------------
    # Parents
    # ------------------------------------------------------------

    b.set_parent("Root", None)
    for f in feature_names:
        if f != "Root":
            b.set_parent(f, "Root")

    # Root mandatory group with all children
    configure_feature(
        b,
        "Root",
        parent_group=None,
        own_group=FMGroupType.MANDATORY,
        n_children=len(feature_names) - 1,
    )

    for f in feature_names:
        if f != "Root":
            configure_feature(
                b,
                f,
                parent_group=FMGroupType.MANDATORY,
                own_group=None,
                n_children=0,
            )

    # ============================================================
    # Implication combinations
    # ============================================================

    # A1  =>  B1
    b.add_require_constraint("A1", ONE, ONE, "B1")

    # !A2 =>  B2
    b.add_require_constraint("A2", ZERO, ONE, "B2")

    # A3  => !B3
    b.add_require_constraint("A3", ONE, ZERO, "B3")

    # !A4 => !B4
    b.add_require_constraint("A4", ZERO, ZERO, "B4")

    # ============================================================
    # Equivalence combinations (each expands to two implications)
    # ============================================================

    # C1 <=> D1
    b.add_require_constraint("C1", ONE, ONE, "D1")
    b.add_require_constraint("D1", ONE, ONE, "C1")

    # !C2 <=> D2
    b.add_require_constraint("C2", ZERO, ONE, "D2")
    b.add_require_constraint("D2", ONE, ZERO, "C2")

    # C3 <=> !D3
    b.add_require_constraint("C3", ONE, ZERO, "D3")
    b.add_require_constraint("D3", ZERO, ONE, "C3")

    # !C4 <=> !D4
    b.add_require_constraint("C4", ZERO, ZERO, "D4")
    b.add_require_constraint("D4", ZERO, ZERO, "C4")

    # ============================================================
    # Excludes combinations
    # Maps to implications
    # ============================================================

    # !(E1 & F1)        →  E1  => !F1
    b.add_require_constraint("E1", ONE, ZERO, "F1")

    # !(!E2 & F2)       → !E2 => !F2
    b.add_require_constraint("E2", ZERO, ZERO, "F2")

    # !(E3 & !F3)       →  E3 =>  F3
    b.add_require_constraint("E3", ONE, ONE, "F3")

    # !(!E4 & !F4)      → !E4 =>  F4
    b.add_require_constraint("E4", ZERO, ONE, "F4")

    return b.build()
