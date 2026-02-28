# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------

import json
from pathlib import Path
import pytest
from cfmtools.core.cfm import (
    CFM,
    CardinalityInterval,
    CfmBuilder,
    SimpleCardinalityInterval,
)
from cfmtools.plugins.export.json import JSON, JsonExporter
from cfmtools.plugins.load.json import JsonLoader
from tests.common import mock_path


def iv(lo: int, hi: int | None):
    return CardinalityInterval([SimpleCardinalityInterval(lo, hi)])


def export_to_json(
    monkeypatch: pytest.MonkeyPatch,
    model: CFM,
) -> JSON:
    """
    Helper that exports a model through the plugin and returns the JSON payload.
    """
    captured: dict[str, str] = {}

    def fake_write_text(self: Path, text: str, encoding: str = "utf-8") -> None:
        captured["text"] = text

    # Patch Path.write_text so no real file is written
    monkeypatch.setattr(Path, "write_text", fake_write_text)

    exporter = JsonExporter(path=Path("model.json"))  # type: ignore
    exporter.export(model)

    assert "text" in captured
    return json.loads(captured["text"])


# ---------------------------------------------------------------------------
# positive tests
# ---------------------------------------------------------------------------


def test_json_exporter_minimal_model(monkeypatch: pytest.MonkeyPatch):
    """
    Single root feature, no constraints.
    """
    builder = CfmBuilder(feature_names=["Root"], root="Root")
    builder.set_feature_instance_cardinality("Root", iv(1, 1))
    builder.set_group_instance_cardinality("Root", iv(0, 0))
    builder.set_group_type_cardinality("Root", iv(0, 0))
    model = builder.build()

    actual = export_to_json(monkeypatch, model)

    expected: JSON = {
        "root": {
            "name": "Root",
            "children": [],
            "instance_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
            "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
        },
        "constraints": [],
    }

    assert actual == expected


def test_json_exporter_tree(monkeypatch: pytest.MonkeyPatch):
    """
    Root
      ├─ A
      └─ B
    """
    builder = CfmBuilder(feature_names=["Root", "A", "B"], root="Root")

    builder.set_parent("A", "Root")
    builder.set_parent("B", "Root")

    builder.set_feature_instance_cardinality("Root", iv(1, 1))
    builder.set_group_instance_cardinality("Root", iv(2, 2))
    builder.set_group_type_cardinality("Root", iv(2, 2))

    for f in ("A", "B"):
        builder.set_feature_instance_cardinality(f, iv(0, 1))
        builder.set_group_instance_cardinality(f, iv(0, 0))
        builder.set_group_type_cardinality(f, iv(0, 0))

    model = builder.build()

    actual = export_to_json(monkeypatch, model)

    expected: JSON = {
        "root": {
            "name": "Root",
            "children": [
                {
                    "name": "A",
                    "children": [],
                    "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
                    "group_instance_cardinality": {
                        "intervals": [{"lower": 0, "upper": 0}]
                    },
                    "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
                },
                {
                    "name": "B",
                    "children": [],
                    "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
                    "group_instance_cardinality": {
                        "intervals": [{"lower": 0, "upper": 0}]
                    },
                    "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
                },
            ],
            "instance_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 2, "upper": 2}]},
            "group_type_cardinality": {"intervals": [{"lower": 2, "upper": 2}]},
        },
        "constraints": [],
    }

    assert actual == expected


def test_json_exporter_require_constraint(monkeypatch: pytest.MonkeyPatch):
    """
    Root
      ├─ A
      └─ B
    A requires B
    """
    builder = CfmBuilder(feature_names=["Root", "A", "B"], root="Root")

    builder.set_parent("A", "Root")
    builder.set_parent("B", "Root")

    builder.set_feature_instance_cardinality("Root", iv(1, 1))
    builder.set_group_instance_cardinality("Root", iv(2, 2))
    builder.set_group_type_cardinality("Root", iv(2, 2))

    for f in ("A", "B"):
        builder.set_feature_instance_cardinality(f, iv(0, 1))
        builder.set_group_instance_cardinality(f, iv(0, 0))
        builder.set_group_type_cardinality(f, iv(0, 0))

    builder.add_require_constraint(
        first_feature="A",
        first_cardinality=iv(1, 1),
        second_cardinality=iv(1, 1),
        second_feature="B",
    )

    model = builder.build()
    actual = export_to_json(monkeypatch, model)

    expected: JSON = {
        "root": {
            "name": "Root",
            "children": [
                {
                    "name": "A",
                    "children": [],
                    "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
                    "group_instance_cardinality": {
                        "intervals": [{"lower": 0, "upper": 0}]
                    },
                    "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
                },
                {
                    "name": "B",
                    "children": [],
                    "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
                    "group_instance_cardinality": {
                        "intervals": [{"lower": 0, "upper": 0}]
                    },
                    "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
                },
            ],
            "instance_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 2, "upper": 2}]},
            "group_type_cardinality": {"intervals": [{"lower": 2, "upper": 2}]},
        },
        "constraints": [
            {
                "require": True,
                "first_feature_name": "A",
                "second_feature_name": "B",
                "first_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
                "second_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            }
        ],
    }

    assert actual == expected


def test_json_exporter_exclude_constraint(monkeypatch: pytest.MonkeyPatch):
    """
    Root
      ├─ A
      └─ B
    A excludes B
    """
    builder = CfmBuilder(feature_names=["Root", "A", "B"], root="Root")

    builder.set_parent("A", "Root")
    builder.set_parent("B", "Root")

    builder.set_feature_instance_cardinality("Root", iv(1, 1))
    builder.set_group_instance_cardinality("Root", iv(2, 2))
    builder.set_group_type_cardinality("Root", iv(2, 2))

    for f in ("A", "B"):
        builder.set_feature_instance_cardinality(f, iv(0, 1))
        builder.set_group_instance_cardinality(f, iv(0, 0))
        builder.set_group_type_cardinality(f, iv(0, 0))

    builder.add_exclude_constraint(
        first_feature="A",
        first_cardinality=iv(1, 1),
        second_cardinality=iv(1, 1),
        second_feature="B",
    )

    model = builder.build()
    actual = export_to_json(monkeypatch, model)

    expected: JSON = {
        "root": {
            "name": "Root",
            "children": [
                {
                    "name": "A",
                    "children": [],
                    "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
                    "group_instance_cardinality": {
                        "intervals": [{"lower": 0, "upper": 0}]
                    },
                    "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
                },
                {
                    "name": "B",
                    "children": [],
                    "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
                    "group_instance_cardinality": {
                        "intervals": [{"lower": 0, "upper": 0}]
                    },
                    "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
                },
            ],
            "instance_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 2, "upper": 2}]},
            "group_type_cardinality": {"intervals": [{"lower": 2, "upper": 2}]},
        },
        "constraints": [
            {
                "require": False,
                "first_feature_name": "A",
                "second_feature_name": "B",
                "first_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
                "second_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            }
        ],
    }

    assert actual == expected


# ---------------------------------------------------------------------------
# round-trip safety
# ---------------------------------------------------------------------------


def test_json_exporter_roundtrip(monkeypatch: pytest.MonkeyPatch):
    """
    Export → load must preserve the model exactly.
    """
    builder = CfmBuilder(feature_names=["Root", "A"], root="Root")
    builder.set_parent("A", "Root")

    builder.set_feature_instance_cardinality("Root", iv(1, 1))
    builder.set_group_instance_cardinality("Root", iv(1, 1))
    builder.set_group_type_cardinality("Root", iv(1, 1))

    builder.set_feature_instance_cardinality("A", iv(0, 1))
    builder.set_group_instance_cardinality("A", iv(0, 0))
    builder.set_group_type_cardinality("A", iv(0, 0))

    model = builder.build()

    # Export
    exported = export_to_json(monkeypatch, model)

    # Load again using real loader logic
    mock_path(monkeypatch, text=json.dumps(exported), exists=True)
    loaded = JsonLoader(Path("roundtrip.json")).load()  # type: ignore

    assert loaded == model
