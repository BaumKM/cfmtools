from typing import Type

from cfmtools.pipeline.core import StepType
from cfmtools.pipeline.export import Exporter
from cfmtools.pipeline.load import Loader
from cfmtools.pipeline.sample import SampleAlgorithm, register_sampling_algorithm
from cfmtools.pipeline.analyze import Analyzer
from cfmtools.pipeline.transform import Transformer
from cfmtools.pluginsystem._registry import register_step


def load(name: str):
    def decorator(cls: Type[Loader]):
        register_step(StepType.LOAD, cls, name=name)
        return cls

    return decorator


def export(name: str):
    def decorator(cls: Type[Exporter]):
        register_step(StepType.EXPORT, cls, name=name)
        return cls

    return decorator


def transform(name: str):
    """
    Decorator for registering a Transform step.
    """

    def decorator(cls: Type[Transformer]):
        register_step(StepType.TRANSFORM, cls, name=name)
        return cls

    return decorator


def analyze(name: str):
    """
    Decorator for registering a Analyze step.
    """

    def decorator(cls: Type[Analyzer]):
        register_step(StepType.ANALYZE, cls, name=name)
        return cls

    return decorator


def sampler(name: str, *, spaces: set[str]):
    """
    Decorator for registering a SamplerPlugin.


    Example:
    @sampler("ranking", spaces={"structural"})
    class RankingSampler(SamplerPlugin):
    ...
    """

    def decorator(cls: Type[SampleAlgorithm]):
        cls.name = name
        cls.supported_config_types = set(spaces)

        register_sampling_algorithm(cls())
        return cls

    return decorator
