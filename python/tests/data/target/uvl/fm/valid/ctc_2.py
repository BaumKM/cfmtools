from cfmtools.core.cfm import CFM, CfmBuilder
from tests.data.target.common_path import TEST_MODEL_PATH
from tests.data.target.fm_helpers import ONE, ZERO, FMGroupType, configure_feature

CTC_2_PATH = TEST_MODEL_PATH / "uvl/fm/valid/ctc_2.uvl"


def ctc_2_cfm() -> CFM:
    feature_names = ["Root", "A", "B"]
    b = CfmBuilder(feature_names=feature_names, root="Root")

    b.set_parent("Root", None)
    b.set_parent("A", "Root")
    b.set_parent("B", "Root")

    configure_feature(b, "Root", None, FMGroupType.MANDATORY, 2)
    configure_feature(b, "A", FMGroupType.MANDATORY, None, 0)
    configure_feature(b, "B", FMGroupType.MANDATORY, None, 0)

    # !A => B
    b.add_require_constraint("A", ZERO, ONE, "B")

    return b.build()
