use std::hash::Hash;
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use crate::model::cfm::CFMCardinalities;
use crate::model::feature::{Feature, FeatureName, FeatureVec};
use crate::model::{
    cfm::{CFM, CfmError, ExcludeConstraint, RequireConstraint},
    interval::CardinalityInterval,
};

#[derive(Debug)]
pub struct CfmBuilder {
    feature_names: FeatureVec<FeatureName>,
    encode: HashMap<FeatureName, Feature>,
    root: Feature,

    parents: FeatureVec<Option<Feature>>,

    feature_instance: FeatureVec<Option<CardinalityInterval>>,
    group_instance: FeatureVec<Option<CardinalityInterval>>,
    group_type: FeatureVec<Option<CardinalityInterval>>,

    require: Vec<RequireConstraint>,
    exclude: Vec<ExcludeConstraint>,
}

impl CfmBuilder {
    pub fn new<N, R>(
        feature_names: impl IntoIterator<Item = N>,
        root: R,
    ) -> Result<Self, BuildError>
    where
        N: Into<String>,
        R: Into<String>,
    {
        let names: FeatureVec<FeatureName> = feature_names
            .into_iter()
            .map(|s| FeatureName::new(s))
            .collect::<Vec<_>>()
            .into();

        if names.is_empty() {
            return Err(BuildError::EmptyFeatureList);
        }

        // Uniqueness check
        let mut seen: HashSet<&str> = HashSet::new();

        for name in &names {
            let s = name.name();
            if !seen.insert(s) {
                return Err(BuildError::DuplicateFeatureName(s.to_string()));
            }
        }

        let root_name = FeatureName::new(root.into());
        let root = names
            .enumerate()
            .find_map(|(idx, name)| (*name == root_name).then_some(idx))
            .ok_or_else(|| BuildError::RootNotInFeatureList(root_name.name().to_string()))?;

        let encode = names
            .enumerate()
            .map(|(feature, name)| (name.clone(), feature))
            .collect::<HashMap<_, _>>();

        let n = names.len();

        Ok(Self {
            feature_names: names,
            encode,
            root,

            parents: vec![None; n].into(),

            feature_instance: vec![None; n].into(),
            group_instance: vec![None; n].into(),
            group_type: vec![None; n].into(),

            require: Vec::new(),
            exclude: Vec::new(),
        })
    }

    // ------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------

    fn lookup(&self, name: &str) -> Result<Feature, BuildError> {
        self.feature_id(name)
            .ok_or_else(|| BuildError::UnknownFeature(name.to_owned()))
    }

    fn feature_id<Q>(&self, name: &Q) -> Option<Feature>
    where
        FeatureName: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.encode.get(name).copied()
    }

    // ------------------------------------------------------------
    // Structure
    // ------------------------------------------------------------

    pub fn set_parent(
        &mut self,
        child_name: impl AsRef<str>,
        parent: Option<impl AsRef<str>>,
    ) -> Result<(), BuildError> {
        let child_name = child_name.as_ref();
        let child = self.lookup(child_name)?;
        let new_parent = parent
            .as_ref()
            .map(|name| self.lookup(name.as_ref()))
            .transpose()?;

        if child == self.root && new_parent.is_some() {
            return Err(BuildError::RootCannotHaveParent);
        }

        if Some(child) == new_parent {
            return Err(BuildError::SelfParenting {
                feature: child_name.to_owned(),
            });
        }

        self.parents[child] = new_parent;

        Ok(())
    }

    // ------------------------------------------------------------
    // Cardinalities
    // ------------------------------------------------------------

    pub fn set_feature_instance_cardinality(
        &mut self,
        feature_name: impl AsRef<str>,
        card: CardinalityInterval,
    ) -> Result<(), BuildError> {
        let feature_name = feature_name.as_ref();
        let feature = self.lookup(feature_name)?;
        let slot = &mut self.feature_instance[feature];

        if slot.is_some() {
            return Err(BuildError::DuplicateCardinality {
                feature: feature_name.to_owned(),
            });
        }

        *slot = Some(card);
        Ok(())
    }

    pub fn set_group_instance_cardinality(
        &mut self,
        feature_name: impl AsRef<str>,
        card: CardinalityInterval,
    ) -> Result<(), BuildError> {
        let feature_name = feature_name.as_ref();
        let feature = self.lookup(feature_name)?;
        let slot = &mut self.group_instance[feature];

        if slot.is_some() {
            return Err(BuildError::DuplicateCardinality {
                feature: feature_name.to_owned(),
            });
        }

        *slot = Some(card);
        Ok(())
    }

    pub fn set_group_type_cardinality(
        &mut self,
        feature_name: impl AsRef<str>,
        card: CardinalityInterval,
    ) -> Result<(), BuildError> {
        let feature_name = feature_name.as_ref();
        let feature = self.lookup(feature_name)?;
        let slot = &mut self.group_type[feature];

        if slot.is_some() {
            return Err(BuildError::DuplicateCardinality {
                feature: feature_name.to_owned(),
            });
        }

        *slot = Some(card);
        Ok(())
    }

    // ------------------------------------------------------------
    // Constraints
    // ------------------------------------------------------------

    pub fn add_require_constraint(
        &mut self,
        from_name: impl AsRef<str>,
        from_cardinality: CardinalityInterval,
        to_cardinality: CardinalityInterval,
        to_name: impl AsRef<str>,
    ) -> Result<(), BuildError> {
        let from_name = from_name.as_ref();
        let to_name = to_name.as_ref();
        let from = self.lookup(from_name)?;
        let to = self.lookup(to_name)?;

        self.require.push(RequireConstraint::new(
            from,
            from_cardinality,
            to_cardinality,
            to,
        ));
        Ok(())
    }

    pub fn add_exclude_constraint(
        &mut self,
        first_name: impl AsRef<str>,
        first_cardinality: CardinalityInterval,
        second_cardinality: CardinalityInterval,
        second_name: impl AsRef<str>,
    ) -> Result<(), BuildError> {
        let first_name = first_name.as_ref();
        let second_name = second_name.as_ref();
        let a = self.lookup(first_name)?;
        let b = self.lookup(second_name)?;

        self.exclude.push(ExcludeConstraint::new(
            a,
            first_cardinality,
            second_cardinality,
            b,
        ));
        Ok(())
    }

    // ------------------------------------------------------------
    // Build
    // ------------------------------------------------------------

    pub fn build(self) -> Result<CFM, CfmError> {
        let fill = |v: FeatureVec<Option<CardinalityInterval>>| -> FeatureVec<CardinalityInterval> {
            v.into_iter()
                .map(|slot| slot.unwrap_or_else(CardinalityInterval::empty))
                .collect::<Vec<_>>()
                .into()
        };
        let cardinalities = CFMCardinalities {
            feature_instance: fill(self.feature_instance),
            group_type: fill(self.group_type),
            group_instance: fill(self.group_instance),
        };

        CFM::try_new(
            self.root,
            self.parents,
            cardinalities,
            self.require,
            self.exclude,
            self.feature_names,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    EmptyFeatureList,
    DuplicateFeatureName(String),
    RootNotInFeatureList(String),
    UnknownFeature(String),

    RootCannotHaveParent,
    SelfParenting { feature: String },

    DuplicateCardinality { feature: String },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFeatureList => {
                write!(f, "feature list must not be empty")
            }

            Self::DuplicateFeatureName(name) => {
                write!(f, "duplicate feature name: '{name}'")
            }

            Self::RootNotInFeatureList(name) => {
                write!(
                    f,
                    "root feature '{name}' is not present in the feature list"
                )
            }

            Self::UnknownFeature(name) => {
                write!(f, "unknown feature '{name}'")
            }

            Self::RootCannotHaveParent => {
                write!(f, "root feature cannot have a parent")
            }

            Self::SelfParenting { feature } => {
                write!(f, "feature '{feature}' cannot be its own parent")
            }

            Self::DuplicateCardinality { feature } => {
                write!(f, "cardinality for feature '{feature}' was already set")
            }
        }
    }
}

impl Error for BuildError {}
