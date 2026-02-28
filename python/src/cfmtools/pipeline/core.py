from abc import ABC, abstractmethod
from dataclasses import dataclass
from enum import Enum


from cfmtools.core.cfm import CFM


class ParamBehavior(Enum):
    """
    Special handling modes for parameters.
    """

    #: Boolean flag: --foo (store_true)
    FLAG = "flag"

    #: Inverted boolean flag: --no-foo (store_false)
    NEGATED_FLAG = "negated_flag"

    #: Repeatable option: --tag A --tag B
    APPEND = "append"

    #: Count occurrences: -v -vv -vvv
    COUNT = "count"

    #: Fixed set of allowed values (choices)
    CHOICE = "choice"

    #: Positional argument instead of --option
    POSITIONAL = "positional"

    #: Hidden from --help
    HIDDEN = "hidden"

    #: Required even if default exists
    REQUIRED = "required"

    #: Parse remaining tokens (argparse.REMAINDER)
    REMAINDER = "remainder"


@dataclass(frozen=True)
class ParamGroup:
    name: str


@dataclass(frozen=True)
class ParamHelp:
    text: str


class StepType(Enum):
    LOAD = "load"
    TRANSFORM = "transform"
    SAMPLE = "sample"
    ANALYZE = "analyze"
    EXPORT = "export"


class PipelineError(RuntimeError):
    pass


class PipelineStep(ABC):
    """
    Base class for all pipeline steps.
    """

    step_type: StepType

    @abstractmethod
    def run(self, context: PipelineContext) -> None: ...

    @classmethod
    @abstractmethod
    def get_step_help(cls) -> str:
        """One-line help for the STEP (e.g. 'load')."""

    @classmethod
    @abstractmethod
    def get_step_description(cls) -> str:
        """Detailed help for the STEP."""

    @classmethod
    def get_command_help(cls) -> str | None:
        """
        One-line help for the COMMAND level.

        By default, commands reuse step-level help.
        Override in concrete plugin commands if needed.
        """
        return cls.get_step_help()

    @classmethod
    def get_command_description(cls) -> str | None:
        """
        Detailed help for the COMMAND level.

        By default, commands reuse step-level description.
        Override in concrete plugin commands if needed.
        """
        return cls.get_step_description()


@dataclass
class Pipeline:
    steps: list[PipelineStep]

    def validate(self) -> None:
        if not self.steps:
            raise PipelineError("Pipeline is empty")

        if self.steps[0].step_type != StepType.LOAD:
            raise PipelineError("Pipeline must start with load")

        for step in self.steps[1:]:
            if step.step_type == StepType.LOAD:
                raise PipelineError("Load may only appear once and first")

    def run(self, context: PipelineContext) -> None:
        self.validate()
        for step in self.steps:
            step.run(context)


class PipelineContext:
    """
    Shared mutable state for one pipeline execution.
    """

    model: CFM | None
    verbose: bool
    pipeline: Pipeline

    def __init__(self, verbose: bool):
        self.verbose = verbose
        self.model = None
        self.pipeline = Pipeline([])
