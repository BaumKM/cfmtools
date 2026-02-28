use std::collections::HashMap;

use cfm_core::model::{
    cfm::{BuildError, CFM, CfmBuilder},
    interval::{CardinalityInterval, SimpleCardinalityInterval},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct PyCfmData {
    version: u32,
    feature_names: Vec<String>,
    root: String,

    // child_name -> parent_name
    parents: HashMap<String, String>,

    // feature_name -> [[lower, upper|null], ...]
    feature_instance_cardinalities: HashMap<String, Vec<(usize, Option<usize>)>>,
    group_instance_cardinalities: HashMap<String, Vec<(usize, Option<usize>)>>,
    group_type_cardinalities: HashMap<String, Vec<(usize, Option<usize>)>>,

    require_constraints: Vec<RequireData>,
    exclude_constraints: Vec<ExcludeData>,
}

#[derive(Deserialize)]
struct RequireData {
    first_feature: String,
    first_cardinality: Vec<(usize, Option<usize>)>,
    second_cardinality: Vec<(usize, Option<usize>)>,
    second_feature: String,
}

#[derive(Deserialize)]
struct ExcludeData {
    first_feature: String,
    first_cardinality: Vec<(usize, Option<usize>)>,
    second_cardinality: Vec<(usize, Option<usize>)>,
    second_feature: String,
}

pub fn build_py_cfm_from_bytes(bytes: &[u8]) -> Result<CFM, String> {
    let data: PyCfmData =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid CFM JSON: {e}"))?;

    if data.version != 1 {
        return Err(format!("Unsupported format version {}", data.version));
    }

    let mut builder = CfmBuilder::new(data.feature_names.clone(), data.root.clone())
        .map_err(|e| e.to_string())?;

    // Parents

    for (child, parent) in data.parents {
        builder
            .set_parent(child, Some(parent))
            .map_err(|e| e.to_string())?;
    }

    // Cardinalities

    apply_cards_map(
        &mut builder,
        &data.feature_instance_cardinalities,
        |b, name, card| b.set_feature_instance_cardinality(name, card),
        "feature_instance_cardinalities",
    )?;

    apply_cards_map(
        &mut builder,
        &data.group_instance_cardinalities,
        |b, name, card| b.set_group_instance_cardinality(name, card),
        "group_instance_cardinalities",
    )?;

    apply_cards_map(
        &mut builder,
        &data.group_type_cardinalities,
        |b, name, card| b.set_group_type_cardinality(name, card),
        "group_type_cardinalities",
    )?;

    // Constraints

    for (idx, require) in data.require_constraints.into_iter().enumerate() {
        builder
            .add_require_constraint(
                require.first_feature,
                parse_card(require.first_cardinality)?,
                parse_card(require.second_cardinality)?,
                require.second_feature,
            )
            .map_err(|e| format!("require_constraints[{idx}]: {e}"))?;
    }

    for (idx, exclude) in data.exclude_constraints.into_iter().enumerate() {
        builder
            .add_exclude_constraint(
                exclude.first_feature,
                parse_card(exclude.first_cardinality)?,
                parse_card(exclude.second_cardinality)?,
                exclude.second_feature,
            )
            .map_err(|e| format!("exclude_constraints[{idx}]: {e}"))?;
    }

    builder.build().map_err(|e| e.to_string())
}

fn apply_cards_map<F>(
    builder: &mut CfmBuilder,
    cards: &HashMap<String, Vec<(usize, Option<usize>)>>,
    setter: F,
    label: &str,
) -> Result<(), String>
where
    F: Fn(&mut CfmBuilder, &str, CardinalityInterval) -> Result<(), BuildError>,
{
    for (name, intervals) in cards {
        let card = parse_card(intervals.clone()).map_err(|e| format!("{label}[{name}]: {e}"))?;

        setter(builder, name.as_str(), card).map_err(|e| format!("{label}[{name}]: {e}"))?;
    }

    Ok(())
}

fn parse_card(intervals: Vec<(usize, Option<usize>)>) -> Result<CardinalityInterval, String> {
    let simple = intervals
        .into_iter()
        .map(|(low, high)| {
            SimpleCardinalityInterval::try_new(low, high)
                .map_err(|e| format!("invalid interval ({low},{high:?}): {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CardinalityInterval::new(simple))
}
