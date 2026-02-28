import inspect
import json
from pathlib import Path
from typing import Annotated, override

from cfmtools.core.cfm import CFM
from cfmtools.pipeline.core import ParamBehavior, ParamHelp
from cfmtools.pipeline.analyze import Analyzer
from cfmtools.pluginsystem import analyze


@analyze("structural-unconstrained")
class UnconstrainedConfigurationSpaceSummary(Analyzer):
    """
    Compute a summary of the unconstrained structural configuration space
    using the native Rust backend.

    The resulting summary is written as a JSON file to the specified output path.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Summarize unconstrained structural configuration space."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Compute a summary of the unconstrained structural configuration space
            using the native Rust backend.

            Cross-tree constraints are ignored.

            The generated JSON report contains structural configuration
            counts and related statistics.

            Optionally, the full feature tree can be included in the output,
            annotated with:
              - number of unconstrained configurations
              - average configuration size per feature

            This analyzer does not modify the model.
        """)

    def __init__(
        self,
        output_path: Annotated[
            Path,
            ParamHelp(
                "Path to the output JSON file where the generated summary will be written."
            ),
        ],
        include_feature_tree: Annotated[
            bool,
            ParamBehavior.FLAG,
            ParamHelp(
                "Include the full feature tree in the output JSON, annotated with "
                "unconstrained configuration counts and average configuration sizes "
                "for each feature."
            ),
        ],
    ) -> None:
        self.output_path = output_path
        self.include_feature_tree = include_feature_tree

    @override
    def analyze(self, model: CFM) -> None:
        structural = model.to_native().structural()

        result = structural.unconstrained_config_space_summary(
            show_full_tree=self.include_feature_tree,
        )

        self.output_path.parent.mkdir(parents=True, exist_ok=True)
        with self.output_path.open("w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)


@analyze("structural-constrained")
class ConstrainedConfigurationSpaceSummary(Analyzer):
    """
    Compute a summary of the constrained structural configuration space
    using the native Rust backend.

    The resulting summary is written as a JSON file to the specified output path.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Summarize constrained structural configuration space."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Compute a summary of the constrained structural configuration space
            using the native Rust backend.

            Cross-tree constraints are respected during computation.

            The computation is time-bounded. If the specified time limit
            is reached, the result may be partial.

            The generated JSON report contains constrained structural
            configuration counts and related statistics.

            Optionally, rank validity information can be included
            in the output, reporting for each rank whether it
            corresponds to a valid configuration.

            This analyzer does not modify the model.
        """)

    def __init__(
        self,
        output_path: Annotated[
            Path,
            ParamHelp(
                "Path to the output JSON file where the generated summary will be written."
            ),
        ],
        time_limit: Annotated[
            int,
            ParamHelp(
                "Maximum time limit in seconds for computing the summary. Must be a positive integer."
            ),
        ],
        show_rank_validity: Annotated[
            bool,
            ParamBehavior.FLAG,
            ParamHelp(
                "Include rank validity information for configurations in the output JSON."
            ),
        ],
    ) -> None:
        if time_limit <= 0:
            raise ValueError("time_limit must be a positive integer")

        self.output_path = output_path
        self.time_limit = time_limit
        self.show_rank_validity = show_rank_validity

    @override
    def analyze(self, model: CFM) -> None:
        structural = model.to_native().structural()

        result = structural.constrained_config_space_summary(
            time_limit_s=self.time_limit,
            show_rank_validity=self.show_rank_validity,
        )

        self.output_path.parent.mkdir(parents=True, exist_ok=True)
        with self.output_path.open("w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)
