import argparse
from importlib.metadata import entry_points
import logging
import sys
import inspect
import textwrap
from typing import Annotated, Protocol, Any, Union, get_args, get_origin

from cfmtools.cli.cli_texts import (
    CLI_DESCRIPTION,
    CLI_EXAMPLES_TEXT,
    CLI_EXECUTION_TEXT,
    CLI_PROG,
    CLI_SECTION_EXAMPLES,
    CLI_SECTION_EXECUTION,
    CLI_SECTION_TYPICAL_FLOW,
    CLI_STEPS_DESCRIPTION,
    CLI_STEPS_METAVAR,
    CLI_STEPS_TITLE,
    CLI_TYPICAL_FLOW_TEXT,
    CLI_USAGE,
    CLI_VERBOSE_HELP,
)
from cfmtools.pipeline.core import (
    ParamBehavior,
    ParamGroup,
    ParamHelp,
    PipelineContext,
    PipelineError,
    PipelineStep,
    StepType,
)
from cfmtools.pluginsystem._registry import RegistryEntry, registry_entries


def configure_logging(verbose: bool) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(levelname)s %(name)s: %(message)s",
    )


class WrappedRawTextHelpFormatter(argparse.HelpFormatter):
    """
    Like the default formatter (wraps to terminal width),
    but preserves explicit newline characters in description,
    epilog, and argument group descriptions.
    """

    def _fill_text(
        self,
        text: str,
        width: int,
        indent: str,
    ) -> str:
        lines: list[str] = text.splitlines()
        output: list[str] = []

        for line in lines:
            if not line.strip():
                # Preserve blank lines
                output.append("")
            else:
                output.append(
                    textwrap.fill(
                        line,
                        width=width,
                        initial_indent=indent,
                        subsequent_indent=indent,
                    )
                )

        return "\n".join(output)


class CommandFactory(Protocol):
    """
    Factory for creating and registering subcommand parsers.


    Obtained from ``ArgumentParser.add_subparsers()`` and used by calling
    ``add_parser(name, ...)`` to attach a new command parser.
    """

    def add_parser(self, name: str, **kwargs) -> argparse.ArgumentParser: ...  # type: ignore


class PipelineParser(argparse.ArgumentParser):
    """
    Argument parser with extra state for dynamic plugin registration.
    """

    # Mapping from StepType to the parser that handles that step.
    step_parsers: dict[StepType, StepParser]

    step_command_factory: CommandFactory


class StepParser(argparse.ArgumentParser):
    """
    Parser for a single pipeline step.
    """

    command_factory: CommandFactory | None


def build_root_parser() -> PipelineParser:
    parser = PipelineParser(
        prog=CLI_PROG,
        usage=CLI_USAGE,
        description=CLI_DESCRIPTION,
        formatter_class=WrappedRawTextHelpFormatter,
    )

    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help=CLI_VERBOSE_HELP,
    )

    # ---- Pipeline steps (subparsers) ----
    step_command_factory = parser.add_subparsers(
        dest="step_type",
        metavar=CLI_STEPS_METAVAR,
        title=CLI_STEPS_TITLE,
        description=CLI_STEPS_DESCRIPTION,
        parser_class=StepParser,
    )

    parser.step_command_factory = step_command_factory
    parser.step_parsers = {}

    # ---- Execution section ----
    execution_group = parser.add_argument_group(CLI_SECTION_EXECUTION)
    execution_group.description = CLI_EXECUTION_TEXT

    # ---- Typical flow section ----
    flow_group = parser.add_argument_group(CLI_SECTION_TYPICAL_FLOW)
    flow_group.description = CLI_TYPICAL_FLOW_TEXT

    # ---- Examples section ----
    examples_group = parser.add_argument_group(CLI_SECTION_EXAMPLES)
    examples_group.description = CLI_EXAMPLES_TEXT

    return parser


def get_step_parser(
    root: PipelineParser,
    step_type: StepType,
) -> StepParser:
    """
    Lazily create parser for each StepType.
    """
    step_parsers = root.step_parsers
    step_command_factory = root.step_command_factory

    if step_type in step_parsers:
        return step_parsers[step_type]

    step_name = step_type.value

    # Let argparse create the parser
    base_step_cls = None
    for _, entry in registry_entries():
        if entry.step_cls.step_type == step_type:
            # Walk MRO to find direct subclass of PipelineStep
            for base in entry.step_cls.__mro__:
                if PipelineStep in base.__bases__:
                    base_step_cls = base
                    break
            break

    step_help = base_step_cls.get_step_help() if base_step_cls else None  # type: ignore
    step_desc = base_step_cls.get_step_description() if base_step_cls else None  # type: ignore

    step_parser: StepParser = step_command_factory.add_parser(  # type: ignore
        step_name,
        help=step_help,
        description=step_desc,
        formatter_class=WrappedRawTextHelpFormatter,
    )

    # Register COMMAND subcommands under this step

    step_parser.command_factory = None
    step_parsers[step_type] = step_parser
    return step_parser


def unwrap_cli_annotation(
    annotation: Any,
) -> tuple[Any, set[ParamBehavior], str | None, str | None]:
    """
    Extract CLI metadata from a type annotation.

    If ``annotation`` is ``typing.Annotated``, returns the base type,
    any ``ParamBehavior`` markers, and an optional help string provided
    via ``ParamHelp``. Otherwise, returns the annotation unchanged.
    """
    origin = get_origin(annotation)

    if origin is Annotated:
        base, *meta = get_args(annotation)

        behaviors = {m for m in meta if isinstance(m, ParamBehavior)}

        help_text = next(
            (m.text for m in meta if isinstance(m, ParamHelp)),
            None,
        )

        group = next(
            (m.name for m in meta if isinstance(m, ParamGroup)),
            None,
        )

        return base, behaviors, help_text, group

    return annotation, set(), None, None


def validate_cli_params(name: str, params: set[ParamBehavior]) -> None:
    """
    Fail fast on incompatible CliParam combinations.
    """
    invalid = [
        (
            {ParamBehavior.FLAG, ParamBehavior.APPEND},
            "FLAG and APPEND cannot be combined",
        ),
        ({ParamBehavior.FLAG, ParamBehavior.POSITIONAL}, "FLAG cannot be positional"),
        (
            {ParamBehavior.COUNT, ParamBehavior.APPEND},
            "COUNT and APPEND cannot be combined",
        ),
        ({ParamBehavior.REMAINDER, ParamBehavior.FLAG}, "REMAINDER cannot be a flag"),
    ]

    for combo, msg in invalid:
        if combo <= params:
            raise ValueError(f"{name}: {msg}")


def register_plugin_command(
    root: PipelineParser,
    step_type: StepType,
    entry: RegistryEntry,
) -> None:
    step_parser = get_step_parser(root, step_type)
    command_factory = step_parser.command_factory

    step_cls = entry.step_cls
    name = entry.name or "default"

    if entry.default:
        # Default command attaches directly to the STEP parser
        target_parser = step_parser

        short_help = step_cls.get_step_help()
        full_help = step_cls.get_step_description()

        target_parser.description = short_help
    else:
        if step_parser.command_factory is None:
            step_parser.command_factory = step_parser.add_subparsers(
                dest="command",
                metavar="COMMAND",
            )
        command_factory = step_parser.command_factory
        # Normal named command
        short_help = step_cls.get_command_help()
        full_help = step_cls.get_command_description()

        target_parser = command_factory.add_parser(  # type: ignore
            name,
            help=short_help,
            description=short_help,  # short text at top
            formatter_class=WrappedRawTextHelpFormatter,
        )

    required_group = target_parser.add_argument_group("required parameters")
    default_optional_group = target_parser.add_argument_group("optional parameters")
    custom_groups: dict[str, argparse._ArgumentGroup] = {}  # type: ignore

    # Introspect __init__ signature and expose parameters as CLI args
    sig = inspect.signature(step_cls.__init__)
    for param in sig.parameters.values():
        if param.name == "self":
            continue

        if param.kind not in (
            inspect.Parameter.POSITIONAL_OR_KEYWORD,
            inspect.Parameter.KEYWORD_ONLY,
        ):
            continue

        # ---- CLI metadata extraction ----
        base_ann, cli_params, annotated_help, group_name = unwrap_cli_annotation(
            param.annotation
        )
        validate_cli_params(param.name, cli_params)

        # ---- argument name ----
        if ParamBehavior.POSITIONAL in cli_params:
            arg_name = param.name
        else:
            arg_name = f"--{param.name.replace('_', '-')}"

        kwargs: dict[str, Any] = {}

        # ---- default / required ----
        has_default = param.default is not inspect.Parameter.empty
        if has_default:
            kwargs["default"] = param.default
        else:
            kwargs["required"] = True

        if ParamBehavior.REQUIRED in cli_params:
            kwargs["required"] = True

        # ---- visibility ----
        if ParamBehavior.HIDDEN in cli_params:
            kwargs["help"] = argparse.SUPPRESS

        # ---- special behaviors ----
        if ParamBehavior.FLAG in cli_params:
            kwargs["action"] = "store_true"
            kwargs.pop("type", None)
            kwargs.pop("required", None)

        elif ParamBehavior.NEGATED_FLAG in cli_params:
            kwargs["action"] = "store_false"
            kwargs.pop("type", None)
            kwargs.pop("required", None)

        elif ParamBehavior.APPEND in cli_params:
            kwargs["action"] = "append"

        elif ParamBehavior.COUNT in cli_params:
            kwargs["action"] = "count"
            kwargs.setdefault("default", 0)

        elif ParamBehavior.REMAINDER in cli_params:
            kwargs["nargs"] = argparse.REMAINDER

        # ---- type inference ----
        ann = base_ann
        origin = get_origin(ann)

        # Optional[T] → T
        if origin is Union:
            args = [a for a in get_args(ann) if a is not type(None)]
            if len(args) == 1:
                ann = args[0]

        if callable(ann) and "action" not in kwargs:
            kwargs["type"] = ann

        # ---- help text ----

        help_text = annotated_help

        # append default if present and meaningful
        if has_default and param.default is not None and "action" not in kwargs:
            default_text = f"(default: {param.default})"
            help_text = f"{help_text} {default_text}" if help_text else default_text

        if help_text and "help" not in kwargs:
            kwargs["help"] = help_text

        is_required = kwargs.get("required", False) and "action" not in kwargs

        # If ParamGroup declared → use custom group
        if group_name:
            if group_name not in custom_groups:
                custom_groups[group_name] = target_parser.add_argument_group(group_name)
            group = custom_groups[group_name]

        # Else fallback to required/default grouping
        else:
            group = required_group if is_required else default_optional_group

        group.add_argument(arg_name, **kwargs)

    # AFTER all arguments are registered:
    if full_help:
        description_group = target_parser.add_argument_group("Description")
        description_group.description = full_help

    target_parser.set_defaults(
        __step_cls__=step_cls,
        __step_type__=step_type,
    )


def register_plugin_commands(root: PipelineParser) -> None:
    for step_type, entry in registry_entries():
        register_plugin_command(root, step_type, entry)


log = logging.getLogger(__name__)


def load_plugins():
    for ep in entry_points(group="cfmtools.plugins"):
        try:
            ep.load()
        except Exception as e:
            log.error("Failed to load plugin %s: %s", ep.name, e)


def split_at_next_step(
    tokens: list[str], step_names: set[str]
) -> tuple[list[str], list[str]]:
    """
    Split tokens into (this_step_tokens, rest_tokens) where rest starts at the next STEP name.
    Assumption: STEP names don't appear as plain argument values.
    """
    if not tokens:
        return [], []

    for i in range(1, len(tokens)):
        if tokens[i] in step_names:
            return tokens[:i], tokens[i:]
    return tokens, []


def run(argv: list[str] | None = None) -> None:
    if argv is None:
        argv = sys.argv[1:]

    load_plugins()

    root = build_root_parser()
    register_plugin_commands(root)

    # Let argparse handle --help normally
    if any(a in ("-h", "--help") for a in argv):
        root.parse_args(argv)
        return

    # First pass: parse global options only
    global_parser = argparse.ArgumentParser(add_help=False)
    global_parser.add_argument("-v", "--verbose", action="store_true")
    global_ns, remaining = global_parser.parse_known_args(argv)

    context = PipelineContext(verbose=global_ns.verbose)
    configure_logging(context.verbose)

    step_names = {st.value for st in StepType}

    # Now parse chained commands
    while remaining:
        if remaining[0] not in step_names:
            root.error(f"Expected a pipeline STEP, got {remaining[0]!r}")

        step_name = remaining[0]
        step_type = StepType(step_name)
        step_parser = root.step_parsers[step_type]

        # Take only tokens belonging to this step (stop at next step token)
        step_tokens, remaining = split_at_next_step(remaining, step_names)
        # step_tokens includes the step name; parse the rest with the step parser
        ns = step_parser.parse_args(step_tokens[1:])

        step_cls = getattr(ns, "__step_cls__", None)
        if step_cls is None:
            root.error("Missing or invalid command")

        kwargs = {
            k: v
            for k, v in vars(ns).items()
            if not k.startswith("_") and k != "command"
        }

        step = step_cls(**kwargs)
        context.pipeline.steps.append(step)

    # Execute pipeline
    try:
        context.pipeline.run(context)
    except PipelineError as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
    except Exception:
        if context.verbose:
            raise
        print("Unexpected error (use --verbose for traceback)", file=sys.stderr)
        sys.exit(2)
