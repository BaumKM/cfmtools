from collections import defaultdict
from dataclasses import dataclass
from typing import Iterator, Type

from cfmtools.pipeline.core import PipelineStep, StepType


@dataclass
class RegistryEntry:
    step_cls: Type[PipelineStep]
    name: str | None
    default: bool


_REGISTRY: dict[StepType, list[RegistryEntry]] = defaultdict(list)


def register_step(
    step_type: StepType,
    step_cls: Type[PipelineStep],
    *,
    name: str | None = None,
    default: bool = False,
):
    if default and name is not None:
        raise ValueError("Default steps must not have a name")

    if not default and not name:
        raise ValueError("Non-default steps must have a name")

    if default:
        # Enforce only one default per step type
        if any(e.default for e in _REGISTRY[step_type]):
            raise RuntimeError(f"Default step already registered for {step_type}")
    _REGISTRY[step_type].append(RegistryEntry(step_cls, name, default))


def registry_entries() -> Iterator[tuple[StepType, RegistryEntry]]:
    for step_type, entries in _REGISTRY.items():
        for entry in entries:
            yield step_type, entry
