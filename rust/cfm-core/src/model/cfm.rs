use std::{error::Error, fmt};

use crate::{
    model::{
        feature::{Feature, FeatureName, FeatureVec},
        interval::CardinalityInterval,
    },
    utils::data_structures::{IndexTree, IndexTreeError, Tree, TreeTraversal},
};
use std::fmt::Write;

mod builder;
pub use builder::*;

#[derive(Clone, Debug)]
pub struct RequireConstraint {
    from: Feature,
    from_cardinality: CardinalityInterval,
    to_cardinality: CardinalityInterval,
    to: Feature,
}

impl RequireConstraint {
    #[must_use]
    pub fn new(
        from: Feature,
        from_cardinality: CardinalityInterval,
        to_cardinality: CardinalityInterval,
        to: Feature,
    ) -> Self {
        Self {
            from,
            from_cardinality,
            to_cardinality,
            to,
        }
    }

    #[must_use]
    pub fn from(&self) -> Feature {
        self.from
    }

    #[must_use]
    pub fn to(&self) -> Feature {
        self.to
    }

    #[must_use]
    pub fn from_cardinality(&self) -> &CardinalityInterval {
        &self.from_cardinality
    }

    #[must_use]
    pub fn to_cardinality(&self) -> &CardinalityInterval {
        &self.to_cardinality
    }
}

#[derive(Clone, Debug)]
pub struct ExcludeConstraint {
    a: Feature,
    a_card: CardinalityInterval,
    b_card: CardinalityInterval,
    b: Feature,
}

impl ExcludeConstraint {
    #[must_use]
    pub fn new(
        a: Feature,
        a_card: CardinalityInterval,
        b_card: CardinalityInterval,
        b: Feature,
    ) -> Self {
        Self {
            a,
            a_card,
            b_card,
            b,
        }
    }

    #[must_use]
    pub fn a(&self) -> Feature {
        self.a
    }

    #[must_use]
    pub fn b(&self) -> Feature {
        self.b
    }

    #[must_use]
    pub fn a_cardinality(&self) -> &CardinalityInterval {
        &self.a_card
    }

    #[must_use]
    pub fn b_cardinality(&self) -> &CardinalityInterval {
        &self.b_card
    }
}

#[derive(Debug, Clone)]
pub struct CFMCardinalities {
    feature_instance: FeatureVec<CardinalityInterval>,
    group_type: FeatureVec<CardinalityInterval>,
    group_instance: FeatureVec<CardinalityInterval>,
}

#[derive(Debug, Clone)]
pub struct CFM {
    feature_tree: IndexTree<Feature>,
    cardinalities: CFMCardinalities,

    require: Vec<RequireConstraint>,
    exclude: Vec<ExcludeConstraint>,

    feature_names: FeatureVec<FeatureName>,
}

impl CFM {
    pub fn try_new(
        root: Feature,
        parents: FeatureVec<Option<Feature>>,
        cardinalities: CFMCardinalities,
        require: Vec<RequireConstraint>,
        exclude: Vec<ExcludeConstraint>,
        feature_names: FeatureVec<FeatureName>,
    ) -> Result<Self, CfmError> {
        Self::build(
            root,
            parents,
            cardinalities,
            feature_names,
            require,
            exclude,
        )
    }

    fn build(
        root: Feature,
        parents: FeatureVec<Option<Feature>>,
        cardinalities: CFMCardinalities,
        feature_names: FeatureVec<FeatureName>,
        require: Vec<RequireConstraint>,
        exclude: Vec<ExcludeConstraint>,
    ) -> Result<Self, CfmError> {
        // ============================================================
        // 0) Try to build feature tree
        // ============================================================

        let feature_tree = IndexTree::try_new(root, parents)?;

        // ============================================================
        // 1) Check lengths + index ranges
        // ============================================================

        let n = feature_tree.size();
        // All feature-indexed sequences must have length n
        if cardinalities.feature_instance.len() != n
            || cardinalities.group_instance.len() != n
            || cardinalities.group_type.len() != n
            || feature_names.len() != n
        {
            return Err(CfmError::InconsistentLengths {
                tree_size: n,
                feature_instance: cardinalities.feature_instance.len(),
                group_instance: cardinalities.group_instance.len(),
                group_type: cardinalities.group_type.len(),
            });
        }

        // ============================================================
        // 2) Cardinalities
        // ============================================================

        // Root FI must be exactly [1,1]
        {
            let root_cardinality = &cardinalities.feature_instance[feature_tree.root()];
            let ok = root_cardinality.contains(1) && root_cardinality.size() == Some(1);
            if !ok {
                return Err(CfmError::InvalidRootCardinality {
                    actual: root_cardinality.clone(),
                });
            }
        }

        // Leaf GI / GT must be exactly [0,0]
        for feature in feature_tree.post_order() {
            if feature_tree.is_leaf(feature) {
                let gi = &cardinalities.group_instance[feature];
                if !(gi.contains(0) && gi.size() == Some(1)) {
                    return Err(CfmError::InvalidLeafGroupInstance {
                        feature: *feature,
                        actual: gi.clone(),
                    });
                }

                let gt = &cardinalities.group_type[feature];
                if !(gt.contains(0) && gt.size() == Some(1)) {
                    return Err(CfmError::InvalidLeafGroupType {
                        feature: *feature,
                        actual: gt.clone(),
                    });
                }
            }
        }

        Ok(Self {
            feature_tree,
            cardinalities,
            require,
            exclude,
            feature_names,
        })
    }

    #[must_use]
    pub fn parent(&self, node: &Feature) -> Option<&Feature> {
        self.feature_tree.parent(node)
    }

    #[must_use]
    pub fn feature_name(&self, feature: &Feature) -> &FeatureName {
        &self.feature_names[feature]
    }

    #[must_use]
    pub fn feature_instance_cardinality(&self, f: &Feature) -> &CardinalityInterval {
        &self.cardinalities.feature_instance[f]
    }

    #[must_use]
    pub fn group_type_cardinality(&self, f: &Feature) -> &CardinalityInterval {
        &self.cardinalities.group_type[f]
    }

    #[must_use]
    pub fn group_instance_cardinality(&self, f: &Feature) -> &CardinalityInterval {
        &self.cardinalities.group_instance[f]
    }

    #[must_use]
    pub fn has_cross_tree_constraints(&self) -> bool {
        !self.require.is_empty() || !self.exclude.is_empty()
    }

    #[must_use]
    pub fn number_of_cross_tree_constraints(&self) -> usize {
        self.exclude.len() + self.require.len()
    }

    /// Returns true iff all cross-tree constraints are satisfied
    /// by the given global feature multiplicities.
    #[must_use]
    pub fn satisfies_cross_tree_constraints(&self, counts: &FeatureVec<usize>) -> bool {
        self.require
            .iter()
            .all(|c| Self::require_satisfied(c, counts))
            && self
                .exclude
                .iter()
                .all(|c| Self::exclude_satisfied(c, counts))
    }

    fn require_satisfied(c: &RequireConstraint, counts: &FeatureVec<usize>) -> bool {
        let from_count = counts[c.from()];
        let to_count = counts[c.to()];

        // If "from" cardinality holds, then "to" must hold.
        if c.from_cardinality().contains(from_count) {
            c.to_cardinality().contains(to_count)
        } else {
            true
        }
    }

    fn exclude_satisfied(c: &ExcludeConstraint, counts: &FeatureVec<usize>) -> bool {
        let a_count = counts[c.a()];
        let b_count = counts[c.b()];

        // Forbidden if both cardinalities hold simultaneously.
        !(c.a_cardinality().contains(a_count) && c.b_cardinality().contains(b_count))
    }

    #[must_use]
    pub fn pretty_print(&self) -> String {
        let mut out = String::new();
        let mut prefix = String::new();

        self.fmt_subtree(self.root(), &mut prefix, true, true, &mut out);

        // Cross-tree constraints
        if self.has_cross_tree_constraints() {
            writeln!(out).unwrap();
            writeln!(out, "Cross-tree constraints:").unwrap();

            for r in &self.require {
                let from = self.feature_name(&r.from());
                let to = self.feature_name(&r.to());

                writeln!(
                    out,
                    "  REQUIRE: {} {} -> {} {}",
                    from.name(),
                    r.from_cardinality(),
                    to.name(),
                    r.to_cardinality()
                )
                .unwrap();
            }

            for e in &self.exclude {
                let a = self.feature_name(&e.a());
                let b = self.feature_name(&e.b());

                writeln!(
                    out,
                    "  EXCLUDE: {} {} x {} {}",
                    a.name(),
                    e.a_cardinality(),
                    b.name(),
                    e.b_cardinality()
                )
                .unwrap();
            }
        }

        out
    }

    fn fmt_subtree(
        &self,
        feature: &Feature,
        prefix: &mut String,
        is_last: bool,
        is_root: bool,
        out: &mut String,
    ) {
        let name = self.feature_name(feature);

        let fi = self.feature_instance_cardinality(feature);
        let gi = self.group_instance_cardinality(feature);
        let gt = self.group_type_cardinality(feature);

        let connector = if is_root {
            ""
        } else if is_last {
            "└── "
        } else {
            "├── "
        };

        writeln!(
            out,
            "{}{}{}  [FI={}, GI={}, GT={}]",
            prefix,
            connector,
            name.name(),
            fi,
            gi,
            gt
        )
        .unwrap();

        let mut children = self.children(feature).peekable();

        let base_len = prefix.len();

        if !is_root {
            if is_last {
                prefix.push_str("    ");
            } else {
                prefix.push_str("│   ");
            }
        }

        while let Some(child) = children.next() {
            let is_last_child = children.peek().is_none();
            self.fmt_subtree(child, prefix, is_last_child, false, out);
        }

        prefix.truncate(base_len);
    }
}

impl Tree for CFM {
    type Node = Feature;
    type Children<'a>
        = <IndexTree<Feature> as Tree>::Children<'a>
    where
        Self: 'a;

    fn root(&self) -> &Self::Node {
        self.feature_tree.root()
    }

    fn children<'a>(&'a self, node: &'a Self::Node) -> Self::Children<'a> {
        self.feature_tree.children(node)
    }

    fn is_leaf(&self, node: &Self::Node) -> bool {
        self.feature_tree.is_leaf(node)
    }

    fn size(&self) -> usize {
        self.feature_tree.size()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfmError {
    IndexTreeError {
        tree_error: IndexTreeError<Feature>,
    },
    InconsistentLengths {
        tree_size: usize,
        feature_instance: usize,
        group_instance: usize,
        group_type: usize,
    },
    InvalidRootCardinality {
        actual: CardinalityInterval,
    },
    InvalidLeafGroupInstance {
        feature: Feature,
        actual: CardinalityInterval,
    },
    InvalidLeafGroupType {
        feature: Feature,
        actual: CardinalityInterval,
    },
}

impl fmt::Display for CfmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InconsistentLengths {
                tree_size,
                feature_instance,
                group_instance,
                group_type,
            } => write!(
                f,
                "inconsistent vector lengths: tree_size={tree_size}, feature_instance={feature_instance}, group_instance={group_instance}, group_type={group_type}"
            ),
            Self::InvalidRootCardinality { actual } => write!(
                f,
                "root feature must have cardinality exactly [1,1], found {actual}"
            ),
            Self::InvalidLeafGroupInstance { feature, actual } => write!(
                f,
                "leaf feature {feature:?} must have group instance cardinality exactly [0,0], found {actual}"
            ),
            Self::InvalidLeafGroupType { feature, actual } => write!(
                f,
                "leaf feature {feature:?} must have group type cardinality exactly [0,0], found {actual}"
            ),
            Self::IndexTreeError { tree_error } => tree_error.fmt(f),
        }
    }
}

impl Error for CfmError {}

impl From<IndexTreeError<Feature>> for CfmError {
    fn from(value: IndexTreeError<Feature>) -> Self {
        Self::IndexTreeError { tree_error: value }
    }
}

#[cfg(test)]
pub mod cfm_test_helpers {

    use crate::model::cfm::CfmBuilder;

    use super::*;

    pub struct CfmHelper;

    impl CfmHelper {
        /// Creates a valid CFM only from feature names.
        ///
        /// The first name is used as root.
        pub fn from_names(names: impl IntoIterator<Item = impl Into<String>>) -> CFM {
            let names_vec: Vec<String> = names.into_iter().map(Into::into).collect();

            let root = names_vec
                .first()
                .expect("at least one feature name required")
                .clone();

            let mut builder = CfmBuilder::new(names_vec.clone(), root.clone())
                .expect("failed to create CfmBuilder");

            Self::apply_valid_cardinalities(&mut builder, &names_vec, &root);

            builder.build().expect("failed to build CFM")
        }

        /// Creates a valid CFM from feature names and `(child, parent)` relations.
        /// The first name is used as the root.
        pub fn from_names_and_parents(
            names: impl IntoIterator<Item = impl Into<String>>,
            relations: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
        ) -> CFM {
            let names_vec: Vec<String> = names.into_iter().map(Into::into).collect();

            let root = names_vec
                .first()
                .cloned()
                .expect("at least one feature name required");

            let relations_vec: Vec<(String, String)> = relations
                .into_iter()
                .map(|(c, p)| (c.into(), p.into()))
                .collect();

            let mut builder = CfmBuilder::new(names_vec.clone(), root.clone())
                .expect("failed to create CfmBuilder");

            // Set parent relations
            for (child, parent) in &relations_vec {
                builder
                    .set_parent(child.as_str(), Some(parent.as_str()))
                    .expect("invalid parent relation");
            }

            Self::apply_valid_cardinalities(&mut builder, &names_vec, &root);

            builder.build().expect("failed to build CFM")
        }

        /// Applies valid cardinalities to the cfm:
        /// - feature instance cardinalities = [1,1]
        /// - all other = [0,0]
        fn apply_valid_cardinalities(
            builder: &mut CfmBuilder,
            feature_names: &[String],
            root: &str,
        ) {
            let zero = CardinalityInterval::empty();
            let one = CardinalityInterval::one();

            // Root FI must be exactly [1,1]
            builder.set_feature_instance_cardinality(root, one).unwrap();

            for name in feature_names {
                builder
                    .set_group_instance_cardinality(name, zero.clone())
                    .unwrap();

                builder
                    .set_group_type_cardinality(name, zero.clone())
                    .unwrap();
            }
        }
    }
}
