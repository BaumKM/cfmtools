from dataclasses import dataclass, field
from enum import Enum, auto
import inspect
import logging
from pathlib import Path
from typing import Annotated, Iterable, override
from cfmtools.core.cfm import (
    CFM,
    CfmBuilder,
    CardinalityInterval,
    SimpleCardinalityInterval,
)
from cfmtools.pipeline.core import ParamHelp
from cfmtools.pipeline.load import Loader
from cfmtools.pluginsystem import loader

from antlr4 import CommonTokenStream, InputStream, ParserRuleContext
from antlr4.error.ErrorListener import ErrorListener
from uvl.UVLCustomLexer import UVLCustomLexer  # type: ignore
from uvl.UVLPythonParserListener import UVLPythonParserListener  # type: ignore
from uvl.UVLPythonParser import UVLPythonParser  # type: ignore

# ============================================================================
# Errors
# ============================================================================


class UVLError(Exception):
    """Base class for all user-facing UVL errors."""


class UVLSyntaxError(UVLError):
    pass


class UVLUnsupportedError(UVLError):
    pass


class _CustomErrorListener(ErrorListener):
    @override
    def syntaxError(
        self,
        recognizer: str,
        offendingSymbol: str,
        line: int,
        column: int,
        msg: str,
        e: Exception | None,
    ):
        raise UVLSyntaxError(f"UVL syntax error at {line}:{column}: {msg}")


# ============================================================================
# Minimal UVL AST
# ============================================================================


class _GroupType(Enum):
    MANDATORY = "mandatory"
    OPTIONAL = "optional"
    OR = "or"
    ALTERNATIVE = "alternative"


@dataclass
class _GroupNode:
    group_type: _GroupType
    children: list[_FeatureNode] = field(default_factory=list["_FeatureNode"])


@dataclass
class _FeatureNode:
    name: str
    groups: list[_GroupNode] = field(default_factory=list["_GroupNode"])


@dataclass(frozen=True, slots=True)
class _Literal:
    name: str
    positive: bool = True

    def negated(self) -> "_Literal":
        return _Literal(self.name, not self.positive)


class _ConstraintKind(Enum):
    IMPLIES = auto()  # litA -> litB
    EQUIVALENT = auto()  # litA <-> litB
    EXCLUDES = auto()  # NOT (litA & litB)


@dataclass(frozen=True, slots=True)
class _Constraint:
    kind: _ConstraintKind
    left: _Literal
    right: _Literal


@dataclass
class _UVLModel:
    root: _FeatureNode | None = None
    constraints: list[_Constraint] = field(default_factory=list[_Constraint])


class _Phase(Enum):
    PREAMBLE = auto()
    FEATURES = auto()
    CONSTRAINTS = auto()


@dataclass
class _Warnings:
    saw_attributes: bool


# ============================================================================
# AST Builder
# ============================================================================


class _UvlBuilder(UVLPythonParserListener):
    """
    Builds a minimal AST and rejects all unsupported constructs.
    """

    def __init__(self):
        self.model = _UVLModel()  # final AST being constructed
        self.warnings = _Warnings(saw_attributes=False)

        self._phase = _Phase.PREAMBLE
        self._group_stack: list[_GroupNode] = []  # stack of open groups
        self._feature_stack: list[_FeatureNode] = []  # stack of open feature nodes
        self._constraint_literals: list[_Literal] = []  # stack of literals
        self._constraint_ops: list[_ConstraintKind] = (
            []
        )  # stack of constraint operators
        self._got_not_for_exclude: bool = (
            False  # tracks if we got a not for a exclude constraint
        )

    # ---------------- Groups ----------------

    def _finalize_group(self, group_type: _GroupType) -> None:
        """
        Finalize the currently open group by:
        - assigning its concrete group type,
        - removing it from the temporary group stack, and
        - attaching it to the active feature in the AST.

        Raises RuntimeError on parser invariant violations.
        """
        if not self._group_stack:
            raise RuntimeError("Parser invariant violated: group stack is empty.")

        if not self._feature_stack:
            raise RuntimeError(
                "Parser invariant violated: group finalized with no active feature."
            )

        group = self._group_stack.pop()
        group.group_type = group_type

        # attach completed group to owning feature
        self._feature_stack[-1].groups.append(group)

    @override
    def enterGroupSpec(self, ctx: UVLPythonParser.GroupSpecContext) -> None:
        # placeholder type, finalized later
        self._group_stack.append(_GroupNode(group_type=_GroupType.MANDATORY))

    @override
    def exitMandatoryGroup(self, ctx: UVLPythonParser.MandatoryGroupContext):
        self._finalize_group(_GroupType.MANDATORY)

    @override
    def exitOptionalGroup(self, ctx: UVLPythonParser.OptionalGroupContext):
        self._finalize_group(_GroupType.OPTIONAL)

    @override
    def exitOrGroup(self, ctx: UVLPythonParser.OrGroupContext):
        self._finalize_group(_GroupType.OR)

    @override
    def exitAlternativeGroup(self, ctx: UVLPythonParser.AlternativeGroupContext):
        self._finalize_group(_GroupType.ALTERNATIVE)

    @override
    def exitCardinalityGroup(self, ctx: UVLPythonParser.CardinalityGroupContext):
        raise UVLUnsupportedError("Group cardinalities are not supported.")

    # ---------------- Features ----------------

    @override
    def enterFeatures(self, ctx: UVLPythonParser.FeaturesContext):
        self._phase = _Phase.FEATURES

    @override
    def exitFeatureCardinality(self, ctx: UVLPythonParser.FeatureCardinalityContext):
        raise UVLUnsupportedError("Feature cardinalities are not supported.")

    @override
    def exitAttributes(self, ctx: UVLPythonParser.AttributesContext):
        self.warnings.saw_attributes = True

    @override
    def exitFeature(self, ctx: UVLPythonParser.FeatureContext):
        # method is only called during features phase
        if not self._feature_stack:
            raise RuntimeError(
                "Parser invariant violated: exiting feature with empty feature stack."
            )
        self._feature_stack.pop()  # remove feature from open features

    @override
    def exitFeatures(self, ctx: UVLPythonParser.FeaturesContext) -> None:
        """
        Validate that all feature and group scopes are closed after parsing
        the feature section.
        """
        self._phase = _Phase.CONSTRAINTS
        if self._group_stack:
            raise RuntimeError(
                "Parser invariant violated: group stack not empty after exiting features."
            )

        if self._feature_stack:
            raise RuntimeError(
                "Parser invariant violated: feature stack not empty after exiting features."
            )

    # ---------------- References ----------------

    @override
    def exitReference(self, ctx: UVLPythonParser.ReferenceContext):
        reference: str = str(ctx.getText())
        match self._phase:
            case _Phase.PREAMBLE:
                raise RuntimeError(
                    f"Parser invariant violated: encountered reference in preamble"
                )
            case _Phase.FEATURES:
                node = _FeatureNode(name=reference)
                if self._group_stack:
                    # child feature
                    self._group_stack[-1].children.append(node)
                else:
                    # root feature
                    if self.model.root is not None:
                        raise RuntimeError(
                            "Parser invariant violated: multiple root features."
                        )
                    self.model.root = node

                # This feature becomes the active parent for nested groups
                self._feature_stack.append(node)
            case _Phase.CONSTRAINTS:
                self._constraint_literals.append(_Literal(reference))
            case _:
                raise AssertionError(f"Unreachable: unknown phase {self.phase}")

    # ---------------- Constraints ----------------

    @override
    def exitImplicationConstraint(
        self, ctx: UVLPythonParser.ImplicationConstraintContext
    ):
        self._constraint_ops.append(_ConstraintKind.IMPLIES)

    @override
    def exitEquivalenceConstraint(
        self, ctx: UVLPythonParser.EquivalenceConstraintContext
    ):
        self._constraint_ops.append(_ConstraintKind.EQUIVALENT)

    @override
    def exitAndConstraint(self, ctx: UVLPythonParser.AndConstraintContext):
        self._constraint_ops.append(_ConstraintKind.EXCLUDES)

    @override
    def exitNotConstraint(self, ctx: UVLPythonParser.NotConstraintContext) -> None:
        # Case 1: no operator yet → negate the literal
        if not self._constraint_ops:
            if not self._constraint_literals:
                raise RuntimeError(
                    "Parser invariant violated: NOT with no literal available."
                )

            last = self._constraint_literals.pop()
            self._constraint_literals.append(last.negated())
            return

        # There is an operator on the stack
        op = self._constraint_ops[-1]

        # Case 2: NOT applied to EXCLUDES → mark flag
        if op == _ConstraintKind.EXCLUDES:
            self._got_not_for_exclude = True
            return

        # Case 3: unsupported NOT usage
        raise UVLUnsupportedError(
            "NOT is only supported for literal negation or in '!(A and B)'."
        )

    @override
    def exitConstraintLine(self, ctx: UVLPythonParser.ConstraintLineContext) -> None:
        # capture and reset got not for exclude
        got_not_for_exclude = self._got_not_for_exclude
        self._got_not_for_exclude = False

        # We only support binary constraints: exactly two references and one operator.
        if len(self._constraint_literals) != 2:
            raise UVLUnsupportedError(
                "Only 'A => B', 'A <=> B' and '!(A and B) are supported, with literals A, B."
            )

        if len(self._constraint_ops) != 1:
            raise UVLUnsupportedError(
                "Only a single operator is allowed in a constraint."
            )

        right = self._constraint_literals.pop()
        left = self._constraint_literals.pop()
        op = self._constraint_ops.pop()

        # Reject attribute constraints like A.x > 5
        if "." in left.name or "." in right.name:
            raise UVLUnsupportedError("Attribute constraints are not supported.")

        # check if we got a not for exclude:
        if op == _ConstraintKind.EXCLUDES and not got_not_for_exclude:
            raise UVLUnsupportedError("'and' is only allowed in '!(A and B)'")

        self.model.constraints.append(
            _Constraint(
                kind=op,
                left=left,
                right=right,
            )
        )

        # ---------------- Unsupported cross-tree constraints ----------------

    def _unsupported_constraint(
        self,
        ctx: ParserRuleContext,
        name: str,
    ) -> None:
        start = ctx.start
        if start is None:
            raise UVLUnsupportedError(
                f"Unsupported cross-tree constraint '{name}' (unknown location)."
            )
        line = start.line
        col = start.column
        raise UVLUnsupportedError(
            f"Unsupported cross-tree constraint '{name}' at {line}:{col}. "
            "Only 'A => B', 'A <=> B' and '!(A and B) are supported, with literals A, B."
        )

    @override
    def exitOrConstraint(self, ctx: UVLPythonParser.OrConstraintContext):
        self._unsupported_constraint(ctx, "or constraint")

    @override
    def exitEquationConstraint(
        self,
        ctx: UVLPythonParser.EquationConstraintContext,
    ) -> None:
        self._unsupported_constraint(ctx, "equation constraint")


# ============================================================================
# Conversion to CFM
# ============================================================================

_ONE = CardinalityInterval([SimpleCardinalityInterval(1, 1)])
_ZERO = CardinalityInterval([SimpleCardinalityInterval(0, 0)])


def _make_fresh_dummy_name(
    parent: str,
    used_names: set[str],
    index: int,
) -> str:
    """
    Create a deterministic, collision-free dummy feature name.
    """
    base = f"dummy_{parent}_{index}"

    if base not in used_names:
        used_names.add(base)
        return base

    i = 1
    while True:
        name = f"{base}_{i}"
        if name not in used_names:
            used_names.add(name)
            return name
        i += 1


def _collect_feature_name_set(
    node: _FeatureNode, acc: set[str] | None = None
) -> set[str]:
    if acc is None:
        acc = set()
    acc.add(node.name)
    for g in node.groups:
        for c in g.children:
            _collect_feature_name_set(c, acc)
    return acc


def _group_sort_key(group: _GroupNode) -> tuple[str, ...]:
    """
    Deterministic ordering key for groups.

    Groups are sorted lexicographically by the sorted list of their
    direct children feature names.
    """
    return tuple(sorted(child.name for child in group.children))


def _normalize_groups(node: _FeatureNode, used_names: set[str]) -> None:
    """
    Normalize a UVL tree so that every feature has at most ONE group.

    If a feature has multiple groups:
      - introduce one mandatory group
      - whose children are dummy features
      - each dummy feature owns exactly one of the original groups
    """

    # normalize recursively
    for g in node.groups:
        for c in g.children:
            _normalize_groups(c, used_names)

    if len(node.groups) <= 1:
        return

    original_groups = sorted(node.groups, key=_group_sort_key)

    dummies: list[_FeatureNode] = []

    for idx, g in enumerate(original_groups):
        dummy_name = _make_fresh_dummy_name(node.name, used_names, idx)
        dummy = _FeatureNode(name=dummy_name)
        dummy.groups = [g]  # move the original group under the dummy
        dummies.append(dummy)

    node.groups = [
        _GroupNode(
            group_type=_GroupType.MANDATORY,
            children=dummies,
        )
    ]


def _feature_instance_cardinality(group: _GroupType) -> CardinalityInterval:
    match group:
        case _GroupType.MANDATORY:
            return _ONE
        case _GroupType.OPTIONAL | _GroupType.OR | _GroupType.ALTERNATIVE:
            return CardinalityInterval([SimpleCardinalityInterval(0, 1)])
        case _:
            raise AssertionError(f"Unreachable: unknown group type {group}")


def _group_instance_cardinality(
    group: _GroupType, n_children: int
) -> CardinalityInterval:
    match group:
        case _GroupType.MANDATORY:
            return CardinalityInterval(
                [SimpleCardinalityInterval(n_children, n_children)]
            )
        case _GroupType.OPTIONAL:
            return CardinalityInterval([SimpleCardinalityInterval(0, n_children)])
        case _GroupType.OR:
            return CardinalityInterval([SimpleCardinalityInterval(1, n_children)])
        case _GroupType.ALTERNATIVE:
            return _ONE
        case _:
            raise AssertionError(f"Unreachable: unknown group type {group}")


def _group_type_cardinality(group: _GroupType, n_children: int) -> CardinalityInterval:
    match group:
        case _GroupType.MANDATORY:
            return CardinalityInterval(
                [SimpleCardinalityInterval(n_children, n_children)]
            )
        case _GroupType.OPTIONAL:
            return CardinalityInterval([SimpleCardinalityInterval(0, n_children)])
        case _GroupType.OR:
            return CardinalityInterval([SimpleCardinalityInterval(1, n_children)])
        case _GroupType.ALTERNATIVE:
            return _ONE
        case _:
            raise AssertionError(f"Unreachable: unknown group type {group}")


def _collect_features(node: _FeatureNode) -> Iterable[str]:
    yield node.name

    for group in node.groups:
        for child in group.children:
            yield from _collect_features(child)


def _build_tree(
    builder: CfmBuilder,
    node: _FeatureNode,
    parent: str | None,
) -> None:
    builder.set_parent(node.name, parent)

    # Leaf
    if not node.groups:
        builder.set_group_instance_cardinality(node.name, _ZERO)
        builder.set_group_type_cardinality(node.name, _ZERO)
        return

    # After normalization there must be exactly one group
    assert len(node.groups) == 1, "Normalization failed: multiple groups remain."

    group = node.groups[0]
    children = group.children
    n = len(children)

    for child in children:
        _build_tree(builder, child, node.name)

    gi = _group_instance_cardinality(group.group_type, n)
    gt = _group_type_cardinality(group.group_type, n)

    builder.set_group_instance_cardinality(node.name, gi)
    builder.set_group_type_cardinality(node.name, gt)

    fi = _feature_instance_cardinality(group.group_type)
    for child in children:
        builder.set_feature_instance_cardinality(child.name, fi)


def _require_literal(
    builder: CfmBuilder,
    a: _Literal,
    b: _Literal,
):

    a_card = _ONE if a.positive else _ZERO
    b_card = _ONE if b.positive else _ZERO

    builder.add_require_constraint(a.name, a_card, b_card, b.name)


def convert_to_cfm(model: _UVLModel) -> CFM:
    if model.root is None:
        raise ValueError("UVL model has no root feature.")

    # normalize multiple groups using dummies
    used_names = _collect_feature_name_set(model.root)
    _normalize_groups(model.root, used_names)

    feature_names = list(_collect_features(model.root))
    builder = CfmBuilder(feature_names=feature_names, root=model.root.name)

    # Root feature instance cardinality = (1,1)
    builder.set_feature_instance_cardinality(model.root.name, _ONE)

    _build_tree(builder, model.root, None)

    # Constraints
    for c in model.constraints:
        match c.kind:
            case _ConstraintKind.IMPLIES:
                _require_literal(builder, c.left, c.right)

            case _ConstraintKind.EQUIVALENT:
                _require_literal(builder, c.left, c.right)
                _require_literal(builder, c.right, c.left)

            case _ConstraintKind.EXCLUDES:
                # NOT (A & B) ≡ A -> !B
                neg_b = c.right.negated()
                _require_literal(builder, c.left, neg_b)

    return builder.build()


# ============================================================================
# Loader plugin
# ============================================================================

log = logging.getLogger(__name__)


@loader("uvl-fm")
class UVLFeatureModelLoader(Loader):
    """
    Load a UVL file as a Boolean feature model.

    Only the structural (Boolean) core of UVL is supported.
    Unsupported constructs are rejected with descriptive errors.
    """

    @classmethod
    @override
    def get_command_help(cls) -> str:
        return "Load a UVL feature model (Boolean core only)."

    @classmethod
    @override
    def get_command_description(cls) -> str:
        return inspect.cleandoc("""
            Load a UVL file and interpret it as a pure Boolean
            feature model.

            Supported constructs:

              - Mandatory and optional features
              - OR and alternative (XOR) groups
              - Binary cross-tree constraints:
                    A => B
                    A <=> B
                    !(A and B)

            Unsupported constructs (rejected with an error):

              - Feature cardinalities
              - Group cardinalities
              - Feature attributes
              - Arithmetic expressions
              - Complex cross-tree constraints

            If attributes are present, they are ignored and
            a warning is emitted.

            The resulting model is converted into an internal CFM
            representation.

            This loader does not modify the input file.
        """)

    def __init__(
        self,
        path: Annotated[
            Path,
            ParamHelp("Path to the UVL feature model file"),
        ],
    ):
        self.path = path

    @override
    def load(self) -> CFM:
        text = self.path.read_text(encoding="utf-8")

        input_stream = InputStream(text)
        lexer = UVLCustomLexer(input_stream)
        lexer.removeErrorListeners()
        lexer.addErrorListener(_CustomErrorListener())

        token_stream = CommonTokenStream(lexer)
        parser = UVLPythonParser(token_stream)
        parser.removeErrorListeners()
        parser.addErrorListener(_CustomErrorListener())

        builder = _UvlBuilder()
        parser.addParseListener(builder)
        parser.featureModel()

        if builder.warnings.saw_attributes:
            log.warning(
                "UVL attributes were ignored while loading '%s'.",
                self.path,
            )

        return convert_to_cfm(builder.model)
