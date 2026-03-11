from pathlib import Path
import json
import sys
from typing import override


from cfmtools.core.cfm import CFM
from cfmtools.pipeline.sample import SampleAlgorithm
from cfmtools.pluginsystem import sampler


@sampler("ranking", spaces={"structural"})
class RankingSampler(SampleAlgorithm):
    """
    Uniform ranking sampler (Rust backend).
    """

    @override
    def run_once(
        self,
        model: CFM,
        *,
        space: str,
        samples: int,
        seed: int | None,
        out_path: Path,
    ) -> None:
        print("This sampler is not implemented yet.", file=sys.stderr)
        raise NotImplementedError("Backtracking sampler is not implemented")

    @override
    def run_benchmark(
        self,
        model: CFM,
        *,
        space: str,
        samples: int,
        runs: int,
        seed: int,
        constrained_space: bool,
        out_path: Path,
    ) -> None:
        structural = model.to_native().structural()

        result = structural.benchmark_ranking_sampler(
            runs=runs,
            samples=samples,
            seed=seed,
            calculate_constrained_space_size=constrained_space,
        )

        out_path.parent.mkdir(parents=True, exist_ok=True)
        with out_path.open("w", encoding="utf-8") as f:
            json.dump(result, f, indent=2)
            f.write("\n")
