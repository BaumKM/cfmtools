import json
from pathlib import Path
import pytest
from cfmtools.core.cfm import (
    CFM,
    CfmBuilder,
    CardinalityInterval,
    FeatureName,
    SimpleCardinalityInterval,
)
from cfmtools.plugins.load.json import (
    JSON,
    JsonLoader,
)
from tests.common import mock_path

# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------


def iv(lo: int, hi: int | None):
    return CardinalityInterval([SimpleCardinalityInterval(lo, hi)])


def load_from_json(
    monkeypatch: pytest.MonkeyPatch, model: JSON, allow_implicit_root: bool = False
) -> CFM:
    """
    Helper that mocks a JSON file and loads it through the plugin.
    """
    mock_path(monkeypatch, text=json.dumps(model), exists=True)
    loader = JsonLoader(
        path=Path("model.json"),  # type: ignore
        allow_implicit_root=allow_implicit_root,  # type: ignore
    )
    return loader.load()


# ---------------------------------------------------------------------------
# positive tests
# --------------------------------------------------------------------------


def test_json_loader_minimal_model(monkeypatch: pytest.MonkeyPatch):
    """
    Single root feature, no constraints.
    """
    model: JSON = {
        "root": {
            "name": "Root",
            "children": [],
            "instance_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
            "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
        },
        "constraints": [],
    }

    actual = load_from_json(monkeypatch, model)

    # Expected model
    builder = CfmBuilder(feature_names=["Root"], root="Root")
    builder.set_feature_instance_cardinality("Root", iv(1, 1))
    builder.set_group_instance_cardinality("Root", iv(0, 0))
    builder.set_group_type_cardinality("Root", iv(0, 0))
    expected = builder.build()

    assert actual == expected


def test_json_loader_tree(monkeypatch: pytest.MonkeyPatch):
    """
    Root
      ├─ A
      └─ B
    """
    model: JSON = {
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

    actual = load_from_json(monkeypatch, model)

    # Expected model
    builder = CfmBuilder(feature_names=["Root", "A", "B"], root="Root")

    builder.set_parent("A", "Root")
    builder.set_parent("B", "Root")

    builder.set_feature_instance_cardinality("Root", iv(1, 1))
    builder.set_group_instance_cardinality("Root", iv(2, 2))
    builder.set_group_type_cardinality("Root", iv(2, 2))

    builder.set_feature_instance_cardinality("A", iv(0, 1))
    builder.set_group_instance_cardinality("A", iv(0, 0))
    builder.set_group_type_cardinality("A", iv(0, 0))

    builder.set_feature_instance_cardinality("B", iv(0, 1))
    builder.set_group_instance_cardinality("B", iv(0, 0))
    builder.set_group_type_cardinality("B", iv(0, 0))

    expected = builder.build()

    assert actual == expected


def test_json_loader_require_constraint(monkeypatch: pytest.MonkeyPatch):
    """
    Root
      ├─ A
      └─ B
    A requires B
    """
    model: JSON = {
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

    actual = load_from_json(monkeypatch, model)

    # Expected model
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

    expected = builder.build()

    assert actual == expected


def test_json_loader_exclude_constraint(monkeypatch: pytest.MonkeyPatch):
    """
    Root
      ├─ A
      └─ B
    A excludes B
    """
    model: JSON = {
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

    actual = load_from_json(monkeypatch, model)

    # Expected model
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

    expected = builder.build()

    assert actual == expected


# ---------------------------------------------------------------------------
# error handling
# ---------------------------------------------------------------------------


def test_json_loader_missing_file(monkeypatch: pytest.MonkeyPatch):
    mock_path(monkeypatch, text="", exists=False)
    loader = JsonLoader(Path("missing.json"))  # type: ignore

    with pytest.raises(FileNotFoundError):
        loader.load()


@pytest.mark.parametrize(
    "bad_model",
    [
        {},  # missing root
        {"root": None},  # invalid root
        {"root": "not-an-object"},  # invalid structure
    ],
)
def test_json_loader_invalid_json(monkeypatch: pytest.MonkeyPatch, bad_model: JSON):
    mock_path(monkeypatch, text=json.dumps(bad_model), exists=True)
    loader = JsonLoader(Path("bad.json"))  # type: ignore

    with pytest.raises(ValueError):
        loader.load()


# ---------------------------------------------------------------------------
# implicit root
# ---------------------------------------------------------------------------


def test_json_loader_root_not_one_one_without_implicit_root(
    monkeypatch: pytest.MonkeyPatch,
):
    model: JSON = {
        "root": {
            "name": "Root",
            "children": [],
            "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
            "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
        },
        "constraints": [],
    }

    with pytest.raises(ValueError):
        load_from_json(monkeypatch, model)


def test_json_loader_implicit_root_optional_root(
    monkeypatch: pytest.MonkeyPatch,
):
    model: JSON = {
        "root": {
            "name": "Root",
            "children": [],
            "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
            "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
        },
        "constraints": [],
    }

    actual = load_from_json(
        monkeypatch,
        model,
        allow_implicit_root=True,
    )

    # Expected model
    builder = CfmBuilder(
        feature_names=["Root_super", "Root"],
        root="Root_super",
    )

    builder.set_parent("Root", "Root_super")

    # artificial root
    builder.set_feature_instance_cardinality("Root_super", iv(1, 1))
    builder.set_group_instance_cardinality("Root_super", iv(0, 1))
    builder.set_group_type_cardinality("Root_super", iv(0, 1))

    # original root
    builder.set_feature_instance_cardinality("Root", iv(0, 1))
    builder.set_group_instance_cardinality("Root", iv(0, 0))
    builder.set_group_type_cardinality("Root", iv(0, 0))

    expected = builder.build()

    assert actual == expected


def test_json_loader_implicit_root_mandatory_root(
    monkeypatch: pytest.MonkeyPatch,
):
    model: JSON = {
        "root": {
            "name": "Root",
            "children": [],
            "instance_cardinality": {"intervals": [{"lower": 1, "upper": 2}]},
            "group_instance_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
            "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
        },
        "constraints": [],
    }

    actual = load_from_json(
        monkeypatch,
        model,
        allow_implicit_root=True,
    )

    builder = CfmBuilder(
        feature_names=["Root_super", "Root"],
        root="Root_super",
    )

    builder.set_parent("Root", "Root_super")

    builder.set_feature_instance_cardinality("Root_super", iv(1, 1))
    builder.set_group_instance_cardinality("Root_super", iv(1, 2))
    builder.set_group_type_cardinality("Root_super", iv(1, 1))

    builder.set_feature_instance_cardinality("Root", iv(1, 2))
    builder.set_group_instance_cardinality("Root", iv(0, 0))
    builder.set_group_type_cardinality("Root", iv(0, 0))

    expected = builder.build()

    assert actual == expected


def test_json_loader_implicit_root_name_collision(
    monkeypatch: pytest.MonkeyPatch,
):
    model: JSON = {
        "root": {
            "name": "Root",
            "children": [
                {
                    "name": "Root_super",
                    "children": [],
                    "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
                    "group_instance_cardinality": {
                        "intervals": [{"lower": 0, "upper": 0}]
                    },
                    "group_type_cardinality": {"intervals": [{"lower": 0, "upper": 0}]},
                }
            ],
            "instance_cardinality": {"intervals": [{"lower": 0, "upper": 1}]},
            "group_instance_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
            "group_type_cardinality": {"intervals": [{"lower": 1, "upper": 1}]},
        },
        "constraints": [],
    }

    actual = load_from_json(
        monkeypatch,
        model,
        allow_implicit_root=True,
    )

    # Root_super already exists → must become Root_super_super
    assert actual.feature_name(actual.root) == FeatureName("Root_super_super")
