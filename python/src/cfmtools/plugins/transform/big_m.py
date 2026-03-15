import inspect
from typing import override
from cfmtools.core.cfm import CFM
from cfmtools.core.transforms.big_m import apply_big_m
from cfmtools.pluginsystem import transformer
from cfmtools.pipeline.transform import Transformer


@transformer("big-m")
class BigMTransform(Transformer):
    """
    Apply Big-M transformation to bound cardinalities.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Apply Big-M transformation to the CFM."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Apply the Big-M transformation to the current CFM model.

            This transformation bounds feature-instance cardinalities
            via the Big-M method to obtain a CFM with finite
            configuration space.
        """)

    @override
    def transform(self, model: CFM) -> CFM:
        return apply_big_m(model)
