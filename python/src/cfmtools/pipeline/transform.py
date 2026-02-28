from abc import abstractmethod
from typing import override
from cfmtools.core.cfm import CFM
from cfmtools.pipeline.core import (
    PipelineContext,
    PipelineError,
    PipelineStep,
    StepType,
)


class Transformer(PipelineStep):
    """
    Base class for all model transformations.

    A Transform:
      - requires a model to already exist in the PipelineContext
      - replaces the model with a transformed one
    """

    step_type = StepType.TRANSFORM

    @classmethod
    @override
    def get_step_help(cls) -> str:
        return "Transform the current CFM model."

    @classmethod
    @override
    def get_step_description(cls) -> str:
        return (
            "Apply a transformation to the current model "
            "and replace it with the transformed result. A model "
            "must be loaded before the transform step is executed."
        )

    @override
    def run(self, context: PipelineContext) -> None:
        if context.model is None:
            raise PipelineError("No model loaded. Transform cannot run.")

        context.model = self.transform(context.model)

    @abstractmethod
    def transform(self, model: CFM) -> CFM:
        """
        Implement the actual transformation.
        Must return a new (or modified) CFM.
        """
        ...
