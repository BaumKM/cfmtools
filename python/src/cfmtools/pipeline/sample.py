from abc import ABC, abstractmethod
from pathlib import Path
from typing import Annotated, Iterable, override


from cfmtools.core.cfm import CFM
from cfmtools.pipeline.core import (
    ParamBehavior,
    ParamGroup,
    ParamHelp,
    PipelineContext,
    PipelineError,
    PipelineStep,
    StepType,
)
from cfmtools.pluginsystem._registry import register_step


class Sampler(PipelineStep):
    """
    Pipeline step responsible for model sampling.

    A Sampler:
      - requires a model to already exist in the PipelineContext
      - selects and executes a registered sampling algorithm
      - generates configurations from the model
      - does not modify the model in the PipelineContext
    """

    step_type = StepType.SAMPLE

    @classmethod
    @override
    def get_step_help(cls) -> str:
        return "Generate samples from the current CFM model."

    @classmethod
    @override
    def get_step_description(cls) -> str:
        return (
            "Execute a sampling algorithm on the current model to produce "
            "configurations.\n\n"
            "Supports single-run sampling and benchmark mode.\n\n"
            "Use --list-algos to inspect available sampling algorithms."
        )

    def __init__(
        self,
        # -------------------------
        # Required (all modes)
        # -------------------------
        algo: Annotated[
            str | None,
            ParamGroup("required (all modes)"),
            ParamHelp("Sampling algorithm name"),
        ] = None,
        output_path: Annotated[
            Path | None,
            ParamGroup("required (all modes)"),
            ParamHelp("Output file for samples or benchmark results"),
        ] = None,
        samples: Annotated[
            int | None,
            ParamGroup("required (all modes)"),
            ParamHelp("Number of samples to generate"),
        ] = None,
        # -------------------------
        # Optional (all modes)
        # -------------------------
        configuration_space: Annotated[
            str | None,
            ParamGroup("optional (all modes)"),
            ParamHelp(
                "Configuration space to sample from (e.g. structural). "
                "Required if the selected algorithm supports multiple spaces."
            ),
        ] = None,
        # -------------------------
        # Sampling mode (default)
        # -------------------------
        seed: Annotated[
            int | None,
            ParamGroup("sampling mode (default)"),
            ParamHelp(
                "Random seed (optional in sampling mode; " "required in benchmark mode)"
            ),
        ] = None,
        # -------------------------
        # Benchmark mode
        # -------------------------
        benchmark: Annotated[
            bool,
            ParamBehavior.FLAG,
            ParamGroup("benchmark mode (--benchmark)"),
            ParamHelp("Enable benchmark mode"),
        ] = False,
        runs: Annotated[
            int | None,
            ParamGroup("benchmark mode (--benchmark)"),
            ParamHelp("Number of benchmark runs (required with --benchmark)"),
        ] = None,
        calculate_constrained_space_size: Annotated[
            bool,
            ParamBehavior.FLAG,
            ParamGroup("benchmark mode (--benchmark)"),
            ParamHelp("Compute exact constrained space size " "(benchmark mode only)"),
        ] = False,
        # -------------------------
        # Introspection
        # -------------------------
        list_algos: Annotated[
            bool,
            ParamBehavior.FLAG,
            ParamGroup("introspection"),
            ParamHelp("List available sampling algorithms and exit"),
        ] = False,
    ):
        self.configuration_space = configuration_space
        self.algo = algo
        self.out = output_path
        self.samples = samples
        self.seed = seed
        self.benchmark = benchmark
        self.runs = runs
        self.list_algos = list_algos
        self.calculate_constrained_space_size = calculate_constrained_space_size

    @override
    def run(self, context: PipelineContext) -> None:
        if context.model is None:
            raise PipelineError("No model loaded")

        if self.list_algos:
            self._print_available_algorithms()
            return

        self._validate_args()

        # checked in validate
        assert self.algo is not None
        assert self.samples is not None
        assert self.out is not None

        sampler = get_sampler(self.algo)
        space = self._resolve_space(sampler)

        if self.benchmark:
            assert self.runs is not None  # checked in _validate_options
            assert self.seed is not None
            sampler.run_benchmark(
                context.model,
                samples=self.samples,
                runs=self.runs,
                seed=self.seed,
                out_path=self.out,
                space=space,
                constrained_space=self.calculate_constrained_space_size,
            )
        else:
            sampler.run_once(
                context.model,
                samples=self.samples,
                seed=self.seed,
                out_path=self.out,
                space=space,
            )

    def _validate_args(self) -> None:
        # Required unless --list is used
        if not self.list_algos:
            if self.algo is None:
                raise PipelineError("--algo is required (unless --list-algos is used)")
            if self.out is None:
                raise PipelineError("--output-path is required")
            if self.samples is None:
                raise PipelineError("--samples is required")

        if self.benchmark:
            if self.runs is None:
                raise PipelineError("--runs is required when --benchmark is enabled")
            if self.seed is None:
                raise PipelineError("--seed is required when --benchmark is enabled")
        else:
            if self.runs is not None:
                raise PipelineError(
                    "--runs may only be used when --benchmark is enabled"
                )
            if self.calculate_constrained_space_size:
                raise PipelineError(
                    "--calculate-constrained-space-size may only be used when --benchmark is enabled"
                )

    def _print_available_algorithms(self) -> None:
        samplers = (
            available_samplers_for(self.configuration_space)
            if self.configuration_space is not None
            else list(available_samplers())
        )
        if not samplers:
            if self.configuration_space is None:
                print("No sampling algorithms registered.")
            else:
                print(
                    f"No sampling algorithms registered for space "
                    f"'{self.configuration_space}'."
                )
            return

        if self.configuration_space is None:
            print("Available sampling algorithms:\n")
        else:
            print(
                f"Available sampling algorithms for space "
                f"'{self.configuration_space}':\n"
            )

        for sampler in sorted(samplers, key=lambda s: s.name):
            supported = ", ".join(sorted(sampler.supported_config_types))
            print(f"  {sampler.name:<15} supports: {supported}")

    def _resolve_space(self, sampler: SampleAlgorithm) -> str:
        """
        Resolve the configuration space to use.
        """
        supported = sorted(sampler.supported_config_types)

        # User explicitly specified a space
        if self.configuration_space is not None:
            if self.configuration_space not in sampler.supported_config_types:
                avail = ", ".join(supported)
                raise ValueError(
                    f"Sampler '{sampler.name}' does not support configuration type "
                    f"'{self.configuration_space}'. Supported: {avail}"
                )
            return self.configuration_space

        # No space specified → try to infer
        if len(supported) == 1:
            return supported[0]

        # Ambiguous
        avail = ", ".join(supported)
        raise ValueError(
            f"Sampler '{sampler.name}' supports multiple configuration types "
            f"({avail}). Please specify --configuration-space."
        )


class SampleAlgorithm(ABC):

    #: CLI-visible name (e.g. "ranking")
    name: str

    #: Supported configuration types (e.g. {"structural", "semi-structural"})
    supported_config_types: set[str]

    @abstractmethod
    def run_once(
        self,
        model: CFM,
        *,
        space: str,
        samples: int,
        seed: int | None,
        out_path: Path,
    ) -> None:
        """
        Run a single sampling execution for the given configuration space.
        """
        raise NotImplementedError

    @abstractmethod
    def run_benchmark(
        self,
        model: CFM,
        *,
        space: str,
        samples: int,
        runs: int,
        seed: int,
        out_path: Path,
        constrained_space: bool,
    ) -> None:
        """
        Run a benchmark execution for the given configuration space.
        """
        raise NotImplementedError


# Internal registry
_ALGORITHMS: dict[str, SampleAlgorithm] = {}
# There is always one sample pipeline step and it can be directly invoked without a name
register_step(StepType.SAMPLE, Sampler, default=True)


def register_sampling_algorithm(algorithm: SampleAlgorithm) -> None:
    """
    Register a sampler plugin.
    """
    name = algorithm.name

    if not name:
        raise ValueError("SamplerPlugin.name must be non-empty")

    if name in _ALGORITHMS:
        raise PipelineError(f"Duplicate sampler plugin '{name}'")

    _ALGORITHMS[name] = algorithm


def get_sampler(name: str) -> SampleAlgorithm:
    """
    Look up a sampler plugin by name.
    """
    try:
        return _ALGORITHMS[name]
    except KeyError:
        available = ", ".join(sorted(_ALGORITHMS.keys()))
        raise KeyError(
            f"Unknown sampler '{name}'. Available samplers: {available}"
        ) from None


def available_samplers() -> Iterable[SampleAlgorithm]:
    """
    Return all registered sampler plugins.
    """
    return _ALGORITHMS.values()


def available_samplers_for(config_type: str | None) -> list[SampleAlgorithm]:
    """
    Return sampler plugins compatible with a given config type.


    If config_type is None, return all samplers.
    """
    samplers = list(_ALGORITHMS.values())

    if config_type is None:
        return samplers

    return [s for s in samplers if config_type in s.supported_config_types]
