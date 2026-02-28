use std::{
    collections::HashMap,
    fmt::{self},
};

use crate::config_spaces::instance::{
    FeatureInstance, InstanceBasedConfigError, InstanceBasedConfiguration,
};

pub struct InstanceBasedConfigurationBuilder {
    // parent relations in FeatureInstance space
    parents: HashMap<FeatureInstance, Option<FeatureInstance>>,
    root: Option<FeatureInstance>,
    num_features: usize,
}

impl InstanceBasedConfigurationBuilder {
    #[must_use]
    pub fn new(num_features: usize) -> Self {
        Self {
            parents: HashMap::new(),
            root: None,
            num_features,
        }
    }

    pub fn set_root(&mut self, root: FeatureInstance) {
        self.root = Some(root);
        self.parents.insert(root, None);
    }

    pub fn set_parent(
        &mut self,
        child: FeatureInstance,
        parent: FeatureInstance,
    ) -> Result<(), BuilderError> {
        if child == parent {
            return Err(BuilderError::SelfParenting { instance: child });
        }

        self.parents.insert(child, Some(parent));
        Ok(())
    }

    pub fn build(self) -> Result<InstanceBasedConfiguration, BuilderError> {
        let root = self.root.ok_or(BuilderError::MissingRoot)?;

        InstanceBasedConfiguration::try_new(self.num_features, root, &self.parents)
            .map_err(Into::into)
    }
}

#[derive(Debug, Clone)]
pub enum BuilderError {
    MissingRoot,
    SelfParenting { instance: FeatureInstance },
    ConfigurationError(InstanceBasedConfigError),
}

impl fmt::Display for BuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRoot => {
                write!(f, "no root instance was specified")
            }
            Self::SelfParenting { instance } => {
                write!(f, "instance {instance:?} cannot be its own parent")
            }
            Self::ConfigurationError(instance_based_config_error) => {
                instance_based_config_error.fmt(f)
            }
        }
    }
}

impl std::error::Error for BuilderError {}

impl From<InstanceBasedConfigError> for BuilderError {
    fn from(value: InstanceBasedConfigError) -> Self {
        Self::ConfigurationError(value)
    }
}
