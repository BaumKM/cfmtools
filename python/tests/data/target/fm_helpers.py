from enum import Enum
from cfmtools.core.cfm import (
    CfmBuilder,
    CardinalityInterval,
    SimpleCardinalityInterval,
)


def CI(lo: int, hi: int | None):
    return CardinalityInterval([SimpleCardinalityInterval(lo, hi)])


ZERO = CI(0, 0)
ONE = CI(1, 1)


class FMGroupType(Enum):
    MANDATORY = "mandatory"
    OPTIONAL = "optional"
    OR = "or"
    ALTERNATIVE = "alternative"


def feature_instance_cardinality(parent_group: FMGroupType | None):
    """
    Cardinality of a feature instance depends on the parent's group type.
    Root has parent_group = None.
    """
    if parent_group is None:
        return ONE
    if parent_group == FMGroupType.MANDATORY:
        return ONE
    if parent_group in (FMGroupType.OPTIONAL, FMGroupType.OR, FMGroupType.ALTERNATIVE):
        return CI(0, 1)
    raise AssertionError(parent_group)


def group_cardinality(group: FMGroupType | None, n_children: int):
    """
    Returns (group_instance_cardinality, group_type_cardinality).
    Leaf has group = None.
    """
    if group is None or n_children == 0:
        return ZERO, ZERO

    if group == FMGroupType.MANDATORY:
        return CI(n_children, n_children), CI(n_children, n_children)

    if group == FMGroupType.OPTIONAL:
        return CI(0, n_children), CI(0, n_children)

    if group == FMGroupType.OR:
        return CI(1, n_children), CI(1, n_children)

    if group == FMGroupType.ALTERNATIVE:
        return ONE, ONE

    raise AssertionError(group)


def configure_feature(
    b: CfmBuilder,
    feature: str,
    parent_group: FMGroupType | None,
    own_group: FMGroupType | None,
    n_children: int,
):
    """
    Configure all cardinalities for a single feature.
    """

    fi = feature_instance_cardinality(parent_group)
    gi, gt = group_cardinality(own_group, n_children)

    b.set_feature_instance_cardinality(feature, fi)
    b.set_group_instance_cardinality(feature, gi)
    b.set_group_type_cardinality(feature, gt)
