from abc import abstractmethod
from typing import override

from cfmtools.core.cfm import CFM
from cfmtools.pipeline.core import (
    PipelineContext,
    PipelineError,
    PipelineStep,
    StepType,
)


class Analyzer(PipelineStep):
    """
    Base class for all model analysis steps.

    An Analyzer:
      - requires a model to already exist in the PipelineContext
      - inspects or evaluates the model without modifying it
    """

    step_type = StepType.ANALYZE

    @classmethod
    @override
    def get_step_help(cls) -> str:
        return "Analyze the current CFM model."

    @classmethod
    @override
    def get_step_description(cls) -> str:
        return (
            "Perform analysis on the current pipeline model and "
            "produce derived information such as metrics or "
            "structural properties. A model must be loaded "
            "before the analyze step is executed."
        )

    @override
    def run(self, context: PipelineContext) -> None:
        if context.model is None:
            raise PipelineError("No model loaded - can't analyze")
        self.analyze(context.model)

    @abstractmethod
    def analyze(self, model: CFM) -> None: ...
