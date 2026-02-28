from abc import abstractmethod
from typing import override
from cfmtools.core.cfm import CFM
from cfmtools.pipeline.core import (
    PipelineContext,
    PipelineError,
    PipelineStep,
    StepType,
)


class Loader(PipelineStep):
    """
    Base class for all model loading steps.

    A Loader:
      - requires no model to exist yet in the PipelineContext
      - creates a model from an external representation
      - stores the created model in the PipelineContext
    """

    step_type = StepType.LOAD

    @override
    def run(self, context: PipelineContext) -> None:
        if context.model is not None:
            raise PipelineError("Model already loaded")
        context.model = self.load()

    @abstractmethod
    def load(self) -> CFM: ...

    @classmethod
    @override
    def get_step_help(cls) -> str:
        return "Load a CFM model into the pipeline."

    @classmethod
    @override
    def get_step_description(cls) -> str:
        return (
            "Initialize the pipeline by constructing a CFM model "
            "from an external representation. Exactly one load step "
            "must precede any transformation, analysis, sampling, "
            "or export step."
        )
