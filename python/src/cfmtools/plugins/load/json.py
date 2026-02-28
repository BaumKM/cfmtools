from dataclasses import dataclass
import inspect
import json
from pathlib import Path
from typing import Annotated, override
from cfmtools.pipeline.core import ParamBehavior, ParamHelp
from cfmtools.pluginsystem import load
from cfmtools.pipeline.load import Loader

from cfmtools.core.cfm import (
    CFM,
    CfmBuilder,
    CardinalityInterval,
    SimpleCardinalityInterval,
)
from cfmtools.util import JSON


def _parse_simple_cardinality_interval(obj: JSON) -> SimpleCardinalityInterval:
    if not isinstance(obj, dict):
        raise ValueError("Interval must be an object")

    lower = obj.get("lower")
    upper = obj.get("upper")

    if not isinstance(lower, int) or isinstance(lower, bool):
        raise ValueError("Interval.lower must be an integer")

    if not (upper is None or (isinstance(upper, int) and not isinstance(upper, bool))):
        raise ValueError("Interval.upper must be an integer or null")

    return SimpleCardinalityInterval(lower=lower, upper=upper)


def _parse_cardinality_interval(obj: JSON) -> CardinalityInterval:
    if not isinstance(obj, dict):
        raise ValueError("Cardinality must be an object")

    intervals: JSON = obj.get("intervals")
    if not isinstance(intervals, list):
        raise ValueError("Cardinality.intervals must be a list")

    parsed = [_parse_simple_cardinality_interval(interval) for interval in intervals]
    return CardinalityInterval(parsed)


@dataclass
class _ParsedFeature:
    name: str
    parent: str | None
    feature_card: CardinalityInterval
    group_instance_card: CardinalityInterval
    group_type_card: CardinalityInterval


@dataclass
class _ParsedConstraint:
    is_require_constraint: bool
    first_feature: str
    first_cardinality: CardinalityInterval
    second_cardinality: CardinalityInterval
    second_feature: str


def _parse_features(
    obj: JSON,
    *,
    parent: str | None,
) -> list[_ParsedFeature]:
    if not isinstance(obj, dict):
        raise ValueError("Feature must be an object")

    result: list[_ParsedFeature] = []

    # Stack entries: (feature_obj, parent_name)
    stack: list[tuple[JSON, str | None]] = [(obj, parent)]

    while stack:
        current_obj, parent_name = stack.pop()

        if not isinstance(current_obj, dict):
            raise ValueError("Feature must be an object")

        name = current_obj.get("name")
        if not isinstance(name, str):
            raise ValueError("Feature.name must be a string")

        children: JSON = current_obj.get("children")
        if not isinstance(children, list):
            raise ValueError("Feature.children must be a list")

        feature_card = _parse_cardinality_interval(
            current_obj.get("instance_cardinality")
        )
        group_instance_card = _parse_cardinality_interval(
            current_obj.get("group_instance_cardinality")
        )
        group_type_card = _parse_cardinality_interval(
            current_obj.get("group_type_cardinality")
        )

        current = _ParsedFeature(
            name=name,
            parent=parent_name,
            feature_card=feature_card,
            group_instance_card=group_instance_card,
            group_type_card=group_type_card,
        )

        result.append(current)

        for child in reversed(children):
            stack.append((child, name))

    return result


def _parse_constraints(
    constraints_obj: JSON,
) -> list[_ParsedConstraint]:
    if not isinstance(constraints_obj, list):
        raise ValueError("constraints must be a list")

    parsed: list[_ParsedConstraint] = []

    for obj in constraints_obj:
        if not isinstance(obj, dict):
            raise ValueError("Constraint must be an object")

        require = obj.get("require")
        if not isinstance(require, bool):
            raise ValueError("Constraint.require must be boolean")

        first = obj.get("first_feature_name")
        second = obj.get("second_feature_name")

        if not isinstance(first, str) or not isinstance(second, str):
            raise ValueError("Constraint feature names must be strings")

        first_card = _parse_cardinality_interval(obj.get("first_cardinality"))
        second_card = _parse_cardinality_interval(obj.get("second_cardinality"))

        parsed.append(
            _ParsedConstraint(
                is_require_constraint=require,
                first_feature=first,
                first_cardinality=first_card,
                second_cardinality=second_card,
                second_feature=second,
            )
        )

    return parsed


def _is_exactly_one(cardinality: CardinalityInterval) -> bool:
    return cardinality.min == 1 and cardinality.max == 1


def _contains_zero(cardinality: CardinalityInterval) -> bool:
    return cardinality.contains(0)


def _make_super_root_name(
    base_name: str,
    existing_names: list[str],
) -> str:
    existing = set(existing_names)
    name = f"{base_name}_super"
    while name in existing:
        name = f"{name}_super"
    return name


def _parse_cfm_from_json(
    obj: JSON,
    allow_implicit_root: bool = False,
) -> CFM:
    if not isinstance(obj, dict):
        raise ValueError("CFM must be a json object")

    root_obj = obj.get("root")
    constraints_obj = obj.get("constraints")

    if root_obj is None:
        raise ValueError("CFM.root missing")

    features = _parse_features(root_obj, parent=None)
    feature_names = [f.name for f in features]

    original_root = features[0]

    if not _is_exactly_one(original_root.feature_card) == 1:
        if not allow_implicit_root:
            raise ValueError("Root feature instance cardinality must be [1,1]")

        artificial_root_name = _make_super_root_name(
            original_root.name,
            feature_names,
        )

        # Determine group type cardinality
        if _contains_zero(original_root.feature_card):
            group_type = CardinalityInterval([SimpleCardinalityInterval(0, 1)])
        else:
            group_type = CardinalityInterval([SimpleCardinalityInterval(1, 1)])

        artificial_root = _ParsedFeature(
            name=artificial_root_name,
            parent=None,
            feature_card=CardinalityInterval([SimpleCardinalityInterval(1, 1)]),
            group_instance_card=original_root.feature_card,
            group_type_card=group_type,
        )

        # Fix original root parent
        original_root.parent = artificial_root_name

        features = [artificial_root] + features
        feature_names = [f.name for f in features]
        root_name = artificial_root_name
    else:
        root_name = original_root.name

    builder = CfmBuilder(
        feature_names=feature_names,
        root=root_name,
    )

    # Tree + cardinalities
    for f in features:
        builder.set_parent(f.name, f.parent)
        builder.set_feature_instance_cardinality(f.name, f.feature_card)
        builder.set_group_instance_cardinality(f.name, f.group_instance_card)
        builder.set_group_type_cardinality(f.name, f.group_type_card)

    # Constraints
    constraints = _parse_constraints(constraints_obj)
    for constraint in constraints:
        if constraint.is_require_constraint:
            builder.add_require_constraint(
                first_feature=constraint.first_feature,
                first_cardinality=constraint.first_cardinality,
                second_cardinality=constraint.second_cardinality,
                second_feature=constraint.second_feature,
            )
        else:
            builder.add_exclude_constraint(
                first_feature=constraint.first_feature,
                first_cardinality=constraint.first_cardinality,
                second_cardinality=constraint.second_cardinality,
                second_feature=constraint.second_feature,
            )
    return builder.build()


@load("json")
class JsonLoader(Loader):
    """
    Load a CFM model from JSON.

    The JSON input must conform to the CFM schema expected by the
    internal parser. Optionally, a synthetic root feature can be
    inferred if the model omits an explicit root.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Load a CFM model from a JSON file."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Load a CFM model from a JSON file.

            The input file must conform to the expected
            JSON structure.

            By default, the model must define an explicit root feature.
            Use --allow-implicit-root to automatically infer a synthetic
            root feature if the JSON structure omits one.

            This loader does not modify the model semantics.
        """)

    def __init__(
        self,
        path: Annotated[
            Path,
            ParamHelp("Path to the input JSON file."),
        ],
        allow_implicit_root: Annotated[
            bool,
            ParamBehavior.FLAG,
            ParamHelp(
                "Allow the JSON model to omit an explicit root feature. "
                "If enabled, a synthetic root feature is inferred automatically."
            ),
        ] = False,
    ):
        self.path = path
        self.allow_implicit_root = allow_implicit_root

    @override
    def load(self) -> CFM:
        if not self.path.exists():
            raise FileNotFoundError(self.path)

        data = json.loads(self.path.read_text(encoding="utf-8"))
        return _parse_cfm_from_json(data, self.allow_implicit_root)
