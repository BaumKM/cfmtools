import inspect
from typing import override

from cfmtools.core.cfm import CFM
from cfmtools.core.transforms.trivial_dead_cardinalities import (
    eliminate_trivial_dead_cardinalities,
)
from cfmtools.pipeline.transform import Transformer
from cfmtools.pluginsystem import transform


@transform("eliminate-trivial-dead-cardinalities")
class EliminateDeadCardinalitiesTransform(Transformer):
    """
    Eliminate trivial dead cardinalities from the model.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Eliminate trivial dead cardinalities."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Eliminate trivially dead cardinalities from the current model.
        """)

    def transform(self, model: CFM) -> CFM:
        return eliminate_trivial_dead_cardinalities(model)
