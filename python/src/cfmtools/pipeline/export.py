from abc import abstractmethod
from typing import override
from cfmtools.core.cfm import CFM
from cfmtools.pipeline.core import (
    PipelineContext,
    PipelineError,
    PipelineStep,
    StepType,
)


class Exporter(PipelineStep):
    """
    Base class for all model export steps.

    An Exporter:
      - requires a model to already exist in the PipelineContext
      - serializes the model into an external representation
      - does not modify the model in the PipelineContext
    """

    step_type = StepType.EXPORT

    @classmethod
    @override
    def get_step_help(cls) -> str:
        return "Export the current CFM model."

    @classmethod
    @override
    def get_step_description(cls) -> str:
        return (
            "Serialize the current pipeline model into an external "
            "representation such as a file or output stream. A model "
            "must be loaded before the export step is executed."
        )

    @override
    def run(self, context: PipelineContext) -> None:
        if context.model is None:
            raise PipelineError("No model loaded - can't export")

        self.export(context.model)

    @abstractmethod
    def export(self, model: CFM) -> None:
        """
        Export the given model to an external representation
        (file, stdout, database, network, etc.)
        """
        ...
