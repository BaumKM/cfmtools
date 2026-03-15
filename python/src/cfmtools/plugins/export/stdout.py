import sys
from typing import override

from cfmtools.core.cfm import CFM
from cfmtools.pipeline.export import Exporter
from cfmtools.pluginsystem import exporter


@exporter("stdout")
class PrettyPrintExporter(Exporter):
    """
    Export a CFM model as a human-readable pretty-printed tree to stdout.
    """

    @override
    def export(self, model: CFM) -> None:
        sys.stdout.write(model.pretty_print())
        sys.stdout.write("\n")
        sys.stdout.flush()
