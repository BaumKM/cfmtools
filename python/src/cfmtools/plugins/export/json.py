import inspect
import json
from pathlib import Path
from typing import Annotated, override

from cfmtools.core.cfm import (
    CFM,
    CardinalityInterval,
    Feature,
    SimpleCardinalityInterval,
)
from cfmtools.pipeline.core import ParamHelp
from cfmtools.pipeline.export import Exporter
from cfmtools.pluginsystem import exporter
from cfmtools.util import JSON

# ---------------------------
# Cardinality serialization
# ---------------------------


def _serialize_simple_interval(iv: SimpleCardinalityInterval) -> JSON:
    return {
        "lower": iv.lower,
        "upper": iv.upper,
    }


def _serialize_cardinality(card: CardinalityInterval) -> JSON:
    return {"intervals": [_serialize_simple_interval(iv) for iv in card]}


# ---------------------------
# Feature tree serialization
# ---------------------------


def _serialize_feature_tree(model: CFM, root: Feature) -> JSON:
    # Stack entries: (feature, visited)
    stack: list[tuple[Feature, bool]] = [(root, False)]
    result_map: dict[Feature, JSON] = {}

    while stack:
        feature, visited = stack.pop()

        if not visited:
            # First time we see this node
            stack.append((feature, True))

            children = list(model.children[feature])

            # Push children to stack (unvisited)
            for child in reversed(children):
                stack.append((child, False))
        else:
            # All children already processed
            name = str(model.feature_name(feature))

            children = list(model.children[feature])

            result_map[feature] = {
                "name": name,
                "instance_cardinality": _serialize_cardinality(
                    model.feature_instance_cardinalities[feature]
                ),
                "group_instance_cardinality": _serialize_cardinality(
                    model.group_instance_cardinalities[feature]
                ),
                "group_type_cardinality": _serialize_cardinality(
                    model.group_type_cardinalities[feature]
                ),
                "children": [result_map[c] for c in children],
            }

    return result_map[root]


# ---------------------------
# Constraints serialization
# ---------------------------


def _serialize_constraints(model: CFM) -> list[JSON]:
    out: list[JSON] = []

    # require constraints
    for c in sorted(
        model.require_constraints,
        key=lambda c: (
            str(model.feature_name(c.first_feature)),
            str(model.feature_name(c.second_feature)),
        ),
    ):
        out.append(
            {
                "require": True,
                "first_feature_name": str(model.feature_name(c.first_feature)),
                "first_cardinality": _serialize_cardinality(c.first_cardinality),
                "second_cardinality": _serialize_cardinality(c.second_cardinality),
                "second_feature_name": str(model.feature_name(c.second_feature)),
            }
        )

    # exclude constraints
    for c in sorted(
        model.exclude_constraints,
        key=lambda c: (
            str(model.feature_name(c.first_feature)),
            str(model.feature_name(c.second_feature)),
        ),
    ):
        out.append(
            {
                "require": False,
                "first_feature_name": str(model.feature_name(c.first_feature)),
                "first_cardinality": _serialize_cardinality(c.first_cardinality),
                "second_cardinality": _serialize_cardinality(c.second_cardinality),
                "second_feature_name": str(model.feature_name(c.second_feature)),
            }
        )

    return out


# ---------------------------
# Top-level serializer
# ---------------------------


def _serialize_cfm(model: CFM) -> JSON:
    return {
        "root": _serialize_feature_tree(model, model.root),
        "constraints": _serialize_constraints(model),
    }


# ---------------------------
# Exporter implementation
# ---------------------------


@exporter("json")
class JsonExporter(Exporter):
    """
    Export the current CFM model to JSON.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Export the model to JSON."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Serialize the current CFM model to a JSON file.

            The exported file contains:

              - the full feature tree structure
              - feature instance cardinalities
              - group instance cardinalities
              - group type cardinalities
              - require and exclude cross-tree constraints

            The output is deterministic and ordered by feature names
            for reproducibility.

            This exporter does not modify the model.
        """)

    def __init__(
        self,
        path: Annotated[
            Path,
            ParamHelp("Output path for the exported JSON file"),
        ],
    ):
        self.path = path

    @override
    def export(self, model: CFM) -> None:
        payload = _serialize_cfm(model)

        self.path.write_text(
            json.dumps(payload, indent=2),
            encoding="utf-8",
        )
