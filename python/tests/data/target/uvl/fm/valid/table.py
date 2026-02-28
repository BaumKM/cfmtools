from cfmtools.core.cfm import CFM, CfmBuilder
from tests.data.target.common_path import TEST_MODEL_PATH
from tests.data.target.fm_helpers import ONE, FMGroupType, configure_feature

TABLE_PATH = TEST_MODEL_PATH / "uvl/fm/valid/table.uvl"


def table_cfm() -> CFM:
    """
    Reference CFM for table.uvl
    """

    feature_names = [
        "Table",
        "dummy_Table_0",
        "dummy_Table_1",
        "Information",
        "DataRelationship",
        "QuantitativeToCategorical",
        "SingleSetOfCategories",
        "MultipleCategories",
        "HierarchicalCategories",
        "QuantitativeToQuantitative",
        "SingleCategoricalItems",
        "MultipleCategoricalItems",
        "TableType",
        "Unidirectional",
        "Bidirectional",
    ]

    b = CfmBuilder(feature_names=feature_names, root="Table")

    # Root
    b.set_parent("Table", None)

    # Table → dummies
    b.set_parent("dummy_Table_0", "Table")
    b.set_parent("dummy_Table_1", "Table")

    # dummy_Table_0 → Information
    b.set_parent("Information", "dummy_Table_0")

    # Information → DataRelationship
    b.set_parent("DataRelationship", "Information")

    # DataRelationship → QuantitativeToCategorical, QuantitativeToQuantitative
    b.set_parent("QuantitativeToCategorical", "DataRelationship")
    b.set_parent("QuantitativeToQuantitative", "DataRelationship")

    # QuantitativeToCategorical → category variants
    b.set_parent("SingleSetOfCategories", "QuantitativeToCategorical")
    b.set_parent("MultipleCategories", "QuantitativeToCategorical")
    b.set_parent("HierarchicalCategories", "QuantitativeToCategorical")

    # QuantitativeToQuantitative → item variants
    b.set_parent("SingleCategoricalItems", "QuantitativeToQuantitative")
    b.set_parent("MultipleCategoricalItems", "QuantitativeToQuantitative")

    # dummy_Table_1 → TableType
    b.set_parent("TableType", "dummy_Table_1")

    # TableType → direction variants
    b.set_parent("Unidirectional", "TableType")
    b.set_parent("Bidirectional", "TableType")

    # ---------------- Table ----------------
    # Root, mandatory group with 2 dummy children
    configure_feature(
        b,
        "Table",
        parent_group=None,
        own_group=FMGroupType.MANDATORY,
        n_children=2,
    )

    # ---------------- dummy_Table_0 ----------------
    # Represents the mandatory {Information} group
    configure_feature(
        b,
        "dummy_Table_0",
        parent_group=FMGroupType.MANDATORY,
        own_group=FMGroupType.MANDATORY,
        n_children=1,
    )

    # ---------------- Information ----------------
    # Mandatory → one child (DataRelationship)
    configure_feature(
        b,
        "Information",
        parent_group=FMGroupType.MANDATORY,
        own_group=FMGroupType.MANDATORY,
        n_children=1,
    )

    # ---------------- DataRelationship ----------------
    # Alternative → two children
    configure_feature(
        b,
        "DataRelationship",
        parent_group=FMGroupType.MANDATORY,
        own_group=FMGroupType.ALTERNATIVE,
        n_children=2,
    )

    # ---------------- QuantitativeToCategorical ----------------
    # Alternative → three children
    configure_feature(
        b,
        "QuantitativeToCategorical",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=FMGroupType.ALTERNATIVE,
        n_children=3,
    )

    # ---------------- SingleSetOfCategories ----------------
    # Leaf under alternative
    configure_feature(
        b,
        "SingleSetOfCategories",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- MultipleCategories ----------------
    # Leaf under alternative
    configure_feature(
        b,
        "MultipleCategories",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- HierarchicalCategories ----------------
    # Leaf under alternative
    configure_feature(
        b,
        "HierarchicalCategories",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- QuantitativeToQuantitative ----------------
    # Alternative → two children
    configure_feature(
        b,
        "QuantitativeToQuantitative",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=FMGroupType.ALTERNATIVE,
        n_children=2,
    )

    # ---------------- SingleCategoricalItems ----------------
    # Leaf under alternative
    configure_feature(
        b,
        "SingleCategoricalItems",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- MultipleCategoricalItems ----------------
    # Leaf under alternative
    configure_feature(
        b,
        "MultipleCategoricalItems",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- dummy_Table_1 ----------------
    # Represents the mandatory {TableType} group
    configure_feature(
        b,
        "dummy_Table_1",
        parent_group=FMGroupType.MANDATORY,
        own_group=FMGroupType.MANDATORY,
        n_children=1,
    )

    # ---------------- TableType ----------------
    # Alternative → two children
    configure_feature(
        b,
        "TableType",
        parent_group=FMGroupType.MANDATORY,
        own_group=FMGroupType.ALTERNATIVE,
        n_children=2,
    )

    # ---------------- Unidirectional ----------------
    # Leaf under alternative
    configure_feature(
        b,
        "Unidirectional",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- Bidirectional ----------------
    # Leaf under alternative
    configure_feature(
        b,
        "Bidirectional",
        parent_group=FMGroupType.ALTERNATIVE,
        own_group=None,
        n_children=0,
    )

    # ---------------- Constraints ----------------
    b.add_require_constraint("SingleSetOfCategories", ONE, ONE, "Unidirectional")
    b.add_require_constraint("SingleCategoricalItems", ONE, ONE, "Unidirectional")

    return b.build()
