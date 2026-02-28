use std::sync::Arc;

use crate::model::{
    cfm::{CFM, CfmBuilder},
    interval::{CardinalityInterval, SimpleCardinalityInterval},
};

fn iv(lo: usize, hi: usize) -> CardinalityInterval {
    CardinalityInterval::new(vec![
        SimpleCardinalityInterval::try_new(lo, Some(hi)).unwrap(),
    ])
}

pub struct TestCFM;

impl TestCFM {
    /// Build a tiny feature model:
    ///
    ///  Root
    ///    ├── A   [0..1]
    ///    └── B   [0..1]
    ///
    /// Configurations:
    ///
    ///     1) Root_1
    ///
    ///     2) Root_1
    ///       └── A_1
    ///
    ///     3) Root_1
    ///       ├── A_1
    ///       └── B_1
    ///
    ///     4) Root_1
    ///       └── B_1
    ///
    #[must_use]
    pub fn build_simple_cfm() -> Arc<crate::model::cfm::CFM> {
        let mut builder =
            CfmBuilder::new(["Root", "A", "B"], "Root").expect("builder creation failed");

        // ---------------------------
        // Structure
        // ---------------------------
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("B", Some("Root")).unwrap();

        // ---------------------------
        // Cardinalities
        // ---------------------------

        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        // A and B are optional: [0..1]
        builder
            .set_feature_instance_cardinality("A", iv(0, 1))
            .unwrap();

        builder
            .set_feature_instance_cardinality("B", iv(0, 1))
            .unwrap();

        // arbitrary types and instances
        builder
            .set_group_type_cardinality("Root", iv(0, 2))
            .unwrap();

        builder
            .set_group_instance_cardinality("Root", iv(0, 2))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build wide feature model
    ///
    ///  Root [1..1]
    ///    ├── A [0..2]
    ///    ├── B [0..2]
    ///    └── C [0..2]
    #[must_use]
    pub fn build_wide_cfm() -> Arc<CFM> {
        let mut builder =
            CfmBuilder::new(["Root", "A", "B", "C"], "Root").expect("builder creation failed");

        // Structure
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("B", Some("Root")).unwrap();
        builder.set_parent("C", Some("Root")).unwrap();

        // Cardinalities
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        // children can repeat up to twice
        builder
            .set_feature_instance_cardinality("A", iv(0, 2))
            .unwrap();
        builder
            .set_feature_instance_cardinality("B", iv(0, 2))
            .unwrap();
        builder
            .set_feature_instance_cardinality("C", iv(0, 2))
            .unwrap();

        // group card bounds (keep permissive)
        builder
            .set_group_type_cardinality("Root", iv(0, 3))
            .unwrap();
        builder
            .set_group_instance_cardinality("Root", iv(0, 6))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build deep model
    ///
    ///  Root [1..1]
    ///    ├── A [0..2]
    ///    │    └── B [0..2]
    ///    │         └── C [0..2]
    ///    └── X [0..2]
    ///         └── Y [0..2]
    ///
    #[must_use]
    pub fn build_deep_cfm() -> Arc<CFM> {
        let mut builder = CfmBuilder::new(["Root", "A", "B", "C", "X", "Y"], "Root")
            .expect("builder creation failed");

        // Structure
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("X", Some("Root")).unwrap();

        builder.set_parent("B", Some("A")).unwrap();
        builder.set_parent("C", Some("B")).unwrap();

        builder.set_parent("Y", Some("X")).unwrap();

        // Cardinalities
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        builder
            .set_feature_instance_cardinality("A", iv(0, 2))
            .unwrap();
        builder
            .set_feature_instance_cardinality("B", iv(0, 2))
            .unwrap();
        builder
            .set_feature_instance_cardinality("C", iv(0, 2))
            .unwrap();

        builder
            .set_feature_instance_cardinality("X", iv(0, 2))
            .unwrap();
        builder
            .set_feature_instance_cardinality("Y", iv(0, 2))
            .unwrap();

        // group card bounds (per feature)
        // Non-leaf features (have children)
        for f in ["Root", "A", "B", "X"] {
            builder.set_group_type_cardinality(f, iv(0, 2)).unwrap();
            builder.set_group_instance_cardinality(f, iv(0, 4)).unwrap();
        }

        // Leaf features (no children)
        for f in ["C", "Y"] {
            builder.set_group_type_cardinality(f, iv(0, 0)).unwrap();
            builder.set_group_instance_cardinality(f, iv(0, 0)).unwrap();
        }
        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build a feature model with unreachable (c, k) combinations ("gaps"):
    ///
    ///  Root [1..1]
    ///    ├── A [0 or 2]
    ///    └── B [0 or 2]
    ///
    /// Group cardinalities on Root:
    ///   - type:     [0..2]
    ///   - instance: [0..2]
    ///
    /// Even though k = 1 is allowed by the group cardinality,
    /// it is not reachable because neither A nor B can appear
    /// with multiplicity 1.
    ///
    /// Reachable configurations:
    ///
    ///  1) `Root_1`
    ///
    ///  2) `Root_1`
    ///     ├── `A_1`
    ///     └── `A_2`
    ///
    ///  3) `Root_1`
    ///     ├── `B_1`
    ///     └── `B_2`
    ///
    #[must_use]
    pub fn build_gap_cfm() -> Arc<CFM> {
        let mut builder =
            CfmBuilder::new(["Root", "A", "B"], "Root").expect("builder creation failed");

        // Structure
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("B", Some("Root")).unwrap();

        // Root always present
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        // GAP: only 0 or 2 allowed (no 1)
        let gap_0_or_2 = CardinalityInterval::new(vec![
            SimpleCardinalityInterval::try_new(0, Some(0)).unwrap(),
            SimpleCardinalityInterval::try_new(2, Some(2)).unwrap(),
        ]);
        builder
            .set_feature_instance_cardinality("A", gap_0_or_2.clone())
            .unwrap();
        builder
            .set_feature_instance_cardinality("B", gap_0_or_2)
            .unwrap();

        // Group cardinalities allow unreachable k = 1
        builder
            .set_group_type_cardinality("Root", iv(0, 2))
            .unwrap();
        builder
            .set_group_instance_cardinality("Root", iv(0, 2))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build a feature model with very large multiplicity gaps:
    ///
    ///  Root [1..1]
    ///    ├── A [0, 5, 10, 1000]
    ///    └── B [0, 5, 10, 1000]
    ///
    /// Group cardinalities on Root:
    ///   - type:     [0..2]
    ///   - instance: [0..2000]
    ///
    /// Many k values are unreachable even though the group bounds allow them.
    ///
    /// Reachable configurations:
    ///   - Root only
    ///   - Root -> A x {5,10,1000}
    ///   - Root -> B x {5,10,1000}
    ///   - Root -> A x x  + B x y, where x,y ∈ {5,10,1000}
    ///
    /// Total = 1 + 3 + 3 + 9 = 16 configurations.
    #[must_use]
    pub fn build_large_gap_cfm() -> Arc<CFM> {
        let mut builder =
            CfmBuilder::new(["Root", "A", "B"], "Root").expect("builder creation failed");

        // Structure
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("B", Some("Root")).unwrap();

        // Root always present
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        // GAP: allowed multiplicities {0, 5, 10, 1000}
        let big_gap = CardinalityInterval::new(vec![
            SimpleCardinalityInterval::try_new(0, Some(0)).unwrap(),
            SimpleCardinalityInterval::try_new(5, Some(5)).unwrap(),
            SimpleCardinalityInterval::try_new(10, Some(10)).unwrap(),
            SimpleCardinalityInterval::try_new(1000, Some(1000)).unwrap(),
        ]);

        builder
            .set_feature_instance_cardinality("A", big_gap.clone())
            .unwrap();
        builder
            .set_feature_instance_cardinality("B", big_gap)
            .unwrap();

        // Group cardinalities intentionally permissive
        builder
            .set_group_type_cardinality("Root", iv(0, 2))
            .unwrap();
        builder
            .set_group_instance_cardinality("Root", iv(0, 2000))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build a feature model with a completely unreachable branch.
    ///
    /// Structure:
    ///
    ///     Root [1..1]
    ///       ├── A [0..1]
    ///       └── X [∅]        (unreachable)
    ///             ├── Y [0..25]
    ///             └── Z [0..25]
    ///
    /// Group cardinalities:
    ///   - Root:
    ///       * type:     [0..2]
    ///       * instance: [0..2]
    ///   - X:
    ///       * type:     [0..50]
    ///       * instance: [0..50]
    ///
    #[must_use]
    pub fn build_cutoff_cfm() -> Arc<CFM> {
        let mut builder =
            CfmBuilder::new(["Root", "A", "X", "Y", "Z"], "Root").expect("builder creation failed");

        // Structure
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("X", Some("Root")).unwrap();
        builder.set_parent("Y", Some("X")).unwrap();
        builder.set_parent("Z", Some("X")).unwrap();

        // Root always present
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        // A is optional
        builder
            .set_feature_instance_cardinality("A", iv(0, 1))
            .unwrap();

        // X is unreachable: empty interval set
        let empty = CardinalityInterval::new(vec![]);
        builder
            .set_feature_instance_cardinality("X", empty)
            .unwrap();

        // Group cardinalities permissive
        builder
            .set_group_type_cardinality("Root", iv(0, 2))
            .unwrap();
        builder
            .set_group_instance_cardinality("Root", iv(0, 2))
            .unwrap();

        // lots of configurations for sub-feature model in X
        builder
            .set_group_instance_cardinality("X", iv(0, 50))
            .unwrap();
        builder.set_group_type_cardinality("X", iv(0, 50)).unwrap();
        builder
            .set_feature_instance_cardinality("Y", iv(0, 25))
            .unwrap();
        builder
            .set_feature_instance_cardinality("Z", iv(0, 25))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build a deep chain of mandatory features ending in a wide optional leaf group.
    ///
    /// Structure:
    ///
    ///     Root [1..1]
    ///       └── A [1..1]
    ///             └── B [1..1]
    ///                   └── C [1..1]
    ///                         └── D [1..1]
    ///                               └── LeafRoot [1..1]
    ///                                     ├── X [0..1]
    ///                                     ├── Y [0..1]
    ///                                     └── Z [0..1]
    ///
    /// Group cardinalities:
    ///   - For all chain nodes (Root, A, B, C, D, LeafRoot):
    ///       * type:     [0..3]
    ///       * instance: [0..3]
    ///   - Leaves (X, Y, Z) have no children.
    ///
    #[must_use]
    pub fn build_deep_chain_cfm() -> Arc<CFM> {
        let mut builder = CfmBuilder::new(
            ["Root", "A", "B", "C", "D", "LeafRoot", "X", "Y", "Z"],
            "Root",
        )
        .expect("builder creation failed");

        // Structure (chain)
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("B", Some("A")).unwrap();
        builder.set_parent("C", Some("B")).unwrap();
        builder.set_parent("D", Some("C")).unwrap();
        builder.set_parent("LeafRoot", Some("D")).unwrap();

        // Wide leaves
        builder.set_parent("X", Some("LeafRoot")).unwrap();
        builder.set_parent("Y", Some("LeafRoot")).unwrap();
        builder.set_parent("Z", Some("LeafRoot")).unwrap();

        // Chain nodes fixed to exactly 1
        for f in ["Root", "A", "B", "C", "D", "LeafRoot"] {
            builder
                .set_feature_instance_cardinality(f, iv(1, 1))
                .unwrap();
            builder.set_group_type_cardinality(f, iv(0, 3)).unwrap();
            builder.set_group_instance_cardinality(f, iv(0, 3)).unwrap();
        }

        // Leaves optional
        for f in ["X", "Y", "Z"] {
            builder
                .set_feature_instance_cardinality(f, iv(0, 1))
                .unwrap();
            builder.set_group_type_cardinality(f, iv(0, 0)).unwrap();
            builder.set_group_instance_cardinality(f, iv(0, 0)).unwrap();
        }

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build a feature model where a parent restricts the total multiplicity
    /// of its children via tight group cardinalities.
    ///
    /// Structure:
    ///
    ///     Root [1..1]
    ///       └── A [1..1]
    ///             ├── X [0..10]
    ///             └── Y [0..10]
    ///
    /// Group cardinalities on A:
    ///   - type:     [0..1]
    ///   - instance: [0..2]
    ///
    #[must_use]
    pub fn build_group_restricted_cfm() -> Arc<CFM> {
        let mut builder =
            CfmBuilder::new(["Root", "A", "X", "Y"], "Root").expect("builder creation failed");

        // Structure
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("X", Some("A")).unwrap();
        builder.set_parent("Y", Some("A")).unwrap();

        // Root
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        builder
            .set_group_instance_cardinality("Root", iv(1, 1))
            .unwrap();
        builder
            .set_group_type_cardinality("Root", iv(1, 1))
            .unwrap();

        // A
        builder
            .set_feature_instance_cardinality("A", iv(1, 1))
            .unwrap();

        // X and Y
        builder
            .set_feature_instance_cardinality("X", iv(0, 10))
            .unwrap();
        builder
            .set_feature_instance_cardinality("Y", iv(0, 10))
            .unwrap();

        // Group on A restricts to at most 2 total instances from one type
        builder.set_group_type_cardinality("A", iv(0, 1)).unwrap();
        builder
            .set_group_instance_cardinality("A", iv(0, 2))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build a feature model where a subtree has no valid configurations,
    /// but the overall model is still satisfiable because that subtree is optional.
    ///
    ///
    /// Structure:
    ///
    ///     Root [1..1]
    ///       ├── A [0..1]          (optional)
    ///       │     └── Dead [1..1]
    ///       │           ├── X [0..1]
    ///       │           └── Y [0..1]
    ///       │
    ///       │   Dead group cardinalities:
    ///       │     - type:     [3..3]
    ///       │     - instance: [3..3]
    ///       │
    ///       │   (Impossible to satisfy because Dead has only two children: X and Y.)
    ///       │
    ///       └── B [1..1]         (mandatory)
    ///
    #[must_use]
    pub fn build_dead_branch_cfm() -> Arc<CFM> {
        let mut builder = CfmBuilder::new(["Root", "A", "Dead", "X", "Y", "B"], "Root")
            .expect("builder creation failed");

        // ---------------------------
        // Structure
        // ---------------------------
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("A", Some("Root")).unwrap();
        builder.set_parent("B", Some("Root")).unwrap();

        builder.set_parent("Dead", Some("A")).unwrap();
        builder.set_parent("X", Some("Dead")).unwrap();
        builder.set_parent("Y", Some("Dead")).unwrap();

        // ---------------------------
        // Feature instance cardinalities
        // ---------------------------

        // Root always present
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        // A optional
        builder
            .set_feature_instance_cardinality("A", iv(0, 1))
            .unwrap();

        // Dead exists if A exists
        builder
            .set_feature_instance_cardinality("Dead", iv(1, 1))
            .unwrap();

        // Children of Dead are optional
        builder
            .set_feature_instance_cardinality("X", iv(0, 1))
            .unwrap();
        builder
            .set_feature_instance_cardinality("Y", iv(0, 1))
            .unwrap();

        // B mandatory
        builder
            .set_feature_instance_cardinality("B", iv(1, 1))
            .unwrap();

        // ---------------------------
        // Group cardinalities
        // ---------------------------

        // Root permissive
        builder
            .set_group_type_cardinality("Root", iv(0, 2))
            .unwrap();
        builder
            .set_group_instance_cardinality("Root", iv(1, 2))
            .unwrap();

        // A
        builder.set_group_type_cardinality("A", iv(1, 1)).unwrap();
        builder
            .set_group_instance_cardinality("A", iv(1, 1))
            .unwrap();

        // Dead
        builder
            .set_group_type_cardinality("Dead", iv(3, 3))
            .unwrap();
        builder
            .set_group_instance_cardinality("Dead", iv(3, 3))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }

    /// Build a feature model with a doubly invalid branch that is cut off
    /// by multiplicity at an upper level.
    ///
    /// Structure:
    ///
    ///     Root [1..1]
    ///       └── Top [0..1]
    ///             └── Mid [1..1]          (invalid)
    ///                   └── Bottom [1..1]   (invalid)
    ///                          └── Leaf [1..1]
    ///
    /// Mid group cardinalities:
    ///   - type:     [1..1]
    ///   - instance: [1..1]
    ///
    /// Bottom group cardinalities:
    ///   - type:     [2..2] -> invalid
    ///   - instance: [1..1]
    #[must_use]
    pub fn build_double_invalid_cutoff_cfm() -> Arc<CFM> {
        let mut builder = CfmBuilder::new(["Root", "Top", "Mid", "Bottom", "Leaf"], "Root")
            .expect("builder creation failed");

        // ---------------------------
        // Structure
        // ---------------------------
        builder.set_parent("Root", None::<&str>).unwrap();
        builder.set_parent("Top", Some("Root")).unwrap();
        builder.set_parent("Mid", Some("Top")).unwrap();
        builder.set_parent("Bottom", Some("Mid")).unwrap();
        builder.set_parent("Leaf", Some("Bottom")).unwrap();

        // ---------------------------
        // Feature instance cardinalities
        // ---------------------------

        // Root mandatory
        builder
            .set_feature_instance_cardinality("Root", iv(1, 1))
            .unwrap();

        // Top optional
        builder
            .set_feature_instance_cardinality("Top", iv(0, 1))
            .unwrap();

        builder
            .set_feature_instance_cardinality("Mid", iv(1, 1))
            .unwrap();

        builder
            .set_feature_instance_cardinality("Bottom", iv(1, 1))
            .unwrap();

        builder
            .set_feature_instance_cardinality("Leaf", iv(1, 1))
            .unwrap();

        // ---------------------------
        // Group cardinalities
        // ---------------------------

        builder
            .set_group_type_cardinality("Root", iv(0, 1))
            .unwrap();
        builder
            .set_group_instance_cardinality("Root", iv(0, 1))
            .unwrap();

        builder.set_group_type_cardinality("Top", iv(1, 1)).unwrap();
        builder
            .set_group_instance_cardinality("Top", iv(1, 1))
            .unwrap();

        builder.set_group_type_cardinality("Mid", iv(1, 1)).unwrap();
        builder
            .set_group_instance_cardinality("Mid", iv(1, 1))
            .unwrap();

        builder
            .set_group_type_cardinality("Bottom", iv(2, 2))
            .unwrap();
        builder
            .set_group_instance_cardinality("Bottom", iv(1, 1))
            .unwrap();

        Arc::new(builder.build().expect("CFM build failed"))
    }
}
