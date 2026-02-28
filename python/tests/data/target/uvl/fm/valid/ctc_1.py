from cfmtools.core.cfm import CFM, CfmBuilder
from tests.data.target.common_path import TEST_MODEL_PATH
from tests.data.target.fm_helpers import ONE, FMGroupType, configure_feature

CTC_1_PATH = TEST_MODEL_PATH / "uvl/fm/valid/ctc_1.uvl"


def ctc_1_cfm() -> CFM:
    feature_names = ["Root", "A", "B"]
    b = CfmBuilder(feature_names=feature_names, root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    configure_feature(
        b, "Root", parent_group=None, own_group=FMGroupType.MANDATORY, n_children=2
    )
    configure_feature(
        b, "A", parent_group=FMGroupType.MANDATORY, own_group=None, n_children=0
    )
    configure_feature(b, "B", FMGroupType.MANDATORY, None, 0)

    # A => B
    b.add_require_constraint("A", ONE, ONE, "B")

    return b.build()
