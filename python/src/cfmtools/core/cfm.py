from collections.abc import Iterable, Iterator
from dataclasses import InitVar, dataclass, field

import json
from typing import Any, Generic, NewType, TypeVar

from cfmtools.util import JSON

Feature = NewType("Feature", int)
FeatureName = NewType("FeatureName", str)


from . import _cfm_native

T = TypeVar("T")


class FeatureList(Generic[T]):
    __slots__ = ("_data",)

    def __init__(self, values: Iterable[T]):
        self._data = list(values)

    def __len__(self) -> int:
        return len(self._data)

    def __getitem__(self, feature: Feature) -> T:
        return self._data[int(feature)]

    def __setitem__(self, feature: Feature, value: T) -> None:
        self._data[int(feature)] = value

    def __iter__(self) -> Iterator[T]:
        return iter(self._data)

    def as_feature_tuple(self) -> FeatureTuple[T]:
        return FeatureTuple(self._data)

    def __eq__(self, other: object) -> bool:
        if isinstance(other, FeatureList):
            return self._data == other._data  # type: ignore
        return NotImplemented

    def __repr__(self) -> str:
        return f"{self.__class__.__name__}({self._data!r})"


class FeatureTuple(Generic[T]):
    __slots__ = ("_data",)

    def __init__(self, data: Iterable[T]):
        self._data = tuple(data)

    def __len__(self) -> int:
        return len(self._data)

    def __getitem__(self, feature: Feature) -> T:
        return self._data[int(feature)]

    def __iter__(self) -> Iterator[T]:
        return iter(self._data)

    def __eq__(self, other: object) -> bool:
        if isinstance(other, FeatureTuple):
            return self._data == other._data  # type: ignore
        return NotImplemented

    def __repr__(self) -> str:
        return f"{self.__class__.__name__}({self._data!r})"


@dataclass(frozen=True)
class SimpleCardinalityInterval:
    """Dataclass representing a simple cardinality interval.

    Invariants:
      - lower >= 0
      - upper is None or lower <= upper
    """

    lower: int
    """Lower bound of the interval."""

    upper: int | None
    """Upper bound of the interval. None if unbounded."""

    def __post_init__(self):
        if self.lower < 0:
            raise ValueError(f"lower must be >= 0 (got {self.lower})")

        if self.upper is not None and self.lower > self.upper:
            raise ValueError(
                f"Invalid interval: lower ({self.lower}) > upper ({self.upper})"
            )

    def contains(self, value: int) -> bool:
        """Check if a value is contained in the cardinality interval."""
        if value < self.lower:
            return False
        if self.upper is None:
            return True
        return value <= self.upper

    @property
    def size(self) -> int | None:
        if self.upper is None:
            return None
        return self.upper - self.lower + 1

    def __str__(self) -> str:
        if self.upper is None:
            return f"[{self.lower}, ∞)"
        return f"[{self.lower}, {self.upper}]"


@dataclass(frozen=True)
class CardinalityInterval:
    """Dataclass representing a compound cardinality interval.

    Invariants:
      - _intervals is non-empty
      - simple cardinality intervals are disjoint and sorted increasingly (normalized)
    """

    intervals: InitVar[Iterable[SimpleCardinalityInterval]]
    """Constructor-only parameter (not stored)."""

    _intervals: tuple[SimpleCardinalityInterval, ...] = field(init=False, repr=False)
    """Normalized list of simple cardinality intervals."""

    def __post_init__(self, intervals: Iterable[SimpleCardinalityInterval]):
        # If empty list → default to [0, 0]
        normalized_intervals = self._normalize(intervals)
        if not normalized_intervals:
            normalized_intervals = (SimpleCardinalityInterval(0, 0),)
        object.__setattr__(self, "_intervals", normalized_intervals)

    def contains(self, value: int) -> bool:
        """Check if a value is contained in the cardinality interval."""

        intervals = self._intervals
        low = 0
        high = len(intervals) - 1

        # search with binary search
        while low <= high:
            middle = (low + high) // 2
            mid_interval = intervals[middle]

            if value < mid_interval.lower:
                high = middle - 1
            elif mid_interval.upper is not None and value > mid_interval.upper:
                low = middle + 1
            else:
                # inside interval
                return True
        return False

    @property
    def size(self) -> int | None:
        """
        Total number of admitted values, or None if infinite.
        """
        total = 0
        for interval in self._intervals:
            s = interval.size
            if s is None:
                return None
            total += s
        return total

    @property
    def min(self) -> int:
        """
        Minimum admitted value.
        """
        return self._intervals[0].lower

    @property
    def max(self) -> int | None:
        """
        Maximum admitted value, or None if infinite.
        """
        last = self._intervals[-1]
        return last.upper

    @property
    def non_convex_bound(self) -> int:
        """
        Return the lower bound b such that all non-convex parts of the cardinality interval lay strictly blow b.

        For bounded intervals, this is the maximum finite value admitted.
        For intervals with an unbounded tail [L, ∞), this is L.
        """
        last = self._intervals[-1]

        # Unbounded tail: [L, ∞)
        if last.upper is None:
            return last.lower

        # Fully bounded: take the maximum finite upper endpoint
        return last.upper

    def values(self) -> Iterator[int]:
        """
        Iterate over all admissible values of the cardinality.
        Infinite if unbounded.
        """
        for iv in self._intervals:
            if iv.upper is None:
                x = iv.lower
                while True:
                    yield x
                    x += 1
            else:
                yield from range(iv.lower, iv.upper + 1)

    def bound(self, max_value: int) -> CardinalityInterval:
        new_intervals: list[SimpleCardinalityInterval] = []

        for interval in self._intervals:
            lower = interval.lower
            if interval.upper is None:
                upper = max_value
            else:
                upper = min(interval.upper, max_value)

            if lower <= upper:
                new_intervals.append(SimpleCardinalityInterval(lower, upper))

        return CardinalityInterval(new_intervals)

    def __str__(self) -> str:
        return "".join(str(iv) for iv in self._intervals)

    @staticmethod
    def _normalize(
        intervals: Iterable[SimpleCardinalityInterval],
    ) -> tuple[SimpleCardinalityInterval, ...]:
        """
        Sort and merge the list of simple cardinality intervals.
        Merge overlapping and adjacent intervals.
        """
        intervals_list = list(intervals)
        if not intervals_list:
            return ()
        # Sort by lower bound
        sorted_intervals = sorted(intervals_list, key=lambda iv: iv.lower)

        merged: list[SimpleCardinalityInterval] = []

        cur_low = sorted_intervals[0].lower
        cur_high = sorted_intervals[0].upper

        for interval in sorted_intervals[1:]:
            # Infinite interval absorbs everything
            if cur_high is None:
                break

            # Overlapping or adjacent?
            if interval.lower <= cur_high + 1:
                # Extend current interval
                if interval.upper is None:
                    cur_high = None
                else:
                    cur_high = max(cur_high, interval.upper)
            else:
                # Jump to next interval
                merged.append(SimpleCardinalityInterval(cur_low, cur_high))
                cur_low = interval.lower
                cur_high = interval.upper

        merged.append(SimpleCardinalityInterval(cur_low, cur_high))
        return tuple(merged)

    def __iter__(self) -> Iterator[SimpleCardinalityInterval]:
        """
        Iterate over simple intervals.
        """
        return iter(self._intervals)


EMPTY_CARDINALITY = CardinalityInterval([SimpleCardinalityInterval(0, 0)])


class CfmBuilder:
    """
    Builder for constructing a CFM using feature names.
    Converts names → integer features internally.
    """

    def __init__(self, feature_names: Iterable[str], root: str):
        names = list(feature_names)
        if len(set(names)) != len(names):
            raise ValueError("Feature names must be unique.")

        if root not in names:
            raise ValueError(f"Root feature '{root}' not in feature list.")

        self.feature_names: FeatureTuple[FeatureName] = FeatureTuple(
            FeatureName(n) for n in names
        )
        self.encode: dict[FeatureName, Feature] = {
            name: Feature(i) for i, name in enumerate(self.feature_names)
        }

        self.root: Feature = self.encode[FeatureName(root)]

        n = len(self.feature_names)
        self.parents: FeatureList[Feature | None] = FeatureList([None] * n)
        self.children: FeatureList[list[Feature]] = FeatureList([[] for _ in range(n)])

        self.feature_instance_cardinalities: FeatureList[CardinalityInterval | None] = (
            FeatureList([None] * n)
        )
        self.group_instance_cardinalities: FeatureList[CardinalityInterval | None] = (
            FeatureList([None] * n)
        )
        self.group_type_cardinalities: FeatureList[CardinalityInterval | None] = (
            FeatureList([None] * n)
        )

        self.require_constraints: list[RequireConstraint] = []
        self.exclude_constraints: list[ExcludeConstraint] = []

    # Tree
    def set_parent(self, child: str, parent: str | None) -> None:
        c = self.encode[FeatureName(child)]
        p = None if parent is None else self.encode[FeatureName(parent)]

        self.parents[c] = p
        if p is not None:
            self.children[p].append(c)

    # Cardinalities
    def set_feature_instance_cardinality(
        self, feature: str, card: CardinalityInterval
    ) -> None:
        self.feature_instance_cardinalities[self.encode[FeatureName(feature)]] = card

    def set_group_instance_cardinality(
        self, feature: str, card: CardinalityInterval
    ) -> None:
        self.group_instance_cardinalities[self.encode[FeatureName(feature)]] = card

    def set_group_type_cardinality(
        self, feature: str, card: CardinalityInterval
    ) -> None:
        self.group_type_cardinalities[self.encode[FeatureName(feature)]] = card

    # Constraints
    def add_require_constraint(
        self,
        first_feature: str,
        first_cardinality: CardinalityInterval,
        second_cardinality: CardinalityInterval,
        second_feature: str,
    ) -> None:
        self.require_constraints.append(
            RequireConstraint(
                self.encode[FeatureName(first_feature)],
                first_cardinality,
                second_cardinality,
                self.encode[FeatureName(second_feature)],
            )
        )

    def add_exclude_constraint(
        self,
        first_feature: str,
        first_cardinality: CardinalityInterval,
        second_cardinality: CardinalityInterval,
        second_feature: str,
    ) -> None:
        self.exclude_constraints.append(
            ExcludeConstraint(
                self.encode[FeatureName(first_feature)],
                first_cardinality,
                second_cardinality,
                self.encode[FeatureName(second_feature)],
            )
        )

    # Build
    def build(self) -> CFM:
        def fill_missing(
            values: Iterable[CardinalityInterval | None],
        ) -> FeatureTuple[CardinalityInterval]:
            """
            Replace None entries by EMPTY_CARDINALITY.
            """
            filled: list[CardinalityInterval] = []

            for value in values:
                if value is None:
                    # default to empty cardinality
                    filled.append(EMPTY_CARDINALITY)
                else:
                    filled.append(value)

            return FeatureTuple(filled)

        return CFM(
            feature_names=self.feature_names,
            root=self.root,
            parents=self.parents.as_feature_tuple(),
            children=FeatureTuple(tuple(c) for c in self.children),
            feature_instance_cardinalities=fill_missing(
                self.feature_instance_cardinalities
            ),
            group_instance_cardinalities=fill_missing(
                self.group_instance_cardinalities
            ),
            group_type_cardinalities=fill_missing(self.group_type_cardinalities),
            require_constraints=self.require_constraints,
            exclude_constraints=self.exclude_constraints,
        )


@dataclass(frozen=True)
class RequireConstraint:
    first_feature: Feature
    first_cardinality: CardinalityInterval
    second_cardinality: CardinalityInterval
    second_feature: Feature


@dataclass(frozen=True)
class ExcludeConstraint:
    first_feature: Feature
    first_cardinality: CardinalityInterval
    second_cardinality: CardinalityInterval
    second_feature: Feature


@dataclass(frozen=True)
class CFM:
    # Tree structure
    root: Feature
    parents: FeatureTuple[Feature | None]
    children: FeatureTuple[tuple[Feature, ...]]

    # Cardinalities
    feature_instance_cardinalities: FeatureTuple[CardinalityInterval]
    group_instance_cardinalities: FeatureTuple[CardinalityInterval]
    group_type_cardinalities: FeatureTuple[CardinalityInterval]

    # Cross-tree constraints
    require_constraints: list[RequireConstraint]
    exclude_constraints: list[ExcludeConstraint]

    # featureid-name mapping
    feature_names: FeatureTuple[FeatureName]
    _encode: dict[FeatureName, Feature] = field(init=False, repr=False)

    def __post_init__(self):
        encode = {name: Feature(i) for i, name in enumerate(self.feature_names)}
        object.__setattr__(self, "_encode", encode)
        self.check_well_formed()

    def is_leaf(self, f: Feature) -> bool:
        return len(self.children[f]) == 0

    def features(self) -> Iterator[Feature]:
        return (Feature(i) for i in range(self.n_features))

    def traverse_postorder(self) -> Iterator[Feature]:
        stack: list[tuple[Feature, bool]] = [(self.root, False)]

        while stack:
            node, visited = stack.pop()

            if visited:
                yield node
            else:
                stack.append((node, True))
                for child in self.children[node]:
                    stack.append((child, False))

    def traverse_preorder(self) -> Iterator[Feature]:
        stack: list[Feature] = [self.root]

        while stack:
            node = stack.pop()
            yield node

            stack.extend(self.children[node])

    def check_well_formed(self) -> None:
        """
        Strong well-formedness checks.

        - Structure must be a rooted tree
        - Root feature must have cardinality (1,1)
        - Every leaf must have cardinality (0,0)
        """

        n = self.n_features

        # 1. Root consistency
        if self.parents[self.root] is not None:
            raise AssertionError("Root feature must not have a parent.")

        # Root must be (1,1)
        root_card = self.feature_instance_cardinalities[self.root]
        if not (root_card.contains(1) and root_card.size == 1):
            raise AssertionError("Root feature must have cardinality exactly (1,1).")

        # 2. Parent structure validity
        parent_count = FeatureList([0] * n)

        for feature in self.features():
            parent = self.parents[feature]
            if parent is None:
                continue

            if not (0 <= int(parent) < n):
                raise AssertionError(
                    f"Invalid parent index for feature {feature}: {parent}"
                )

            parent_count[feature] += 1

        # Every non-root must have exactly one parent
        for feature in self.features():
            if Feature(feature) == self.root:
                continue
            if parent_count[feature] != 1:
                raise AssertionError(
                    f"Feature {self.feature_name(feature)} must have exactly one parent."
                )

        # 3. Children consistency
        for feature in self.features():
            children = self.children[feature]
            for child in children:
                if self.parents[child] != feature:
                    raise AssertionError(
                        f"Inconsistent parent/child relation: "
                        f"{self.feature_name(child)} is listed under {self.feature_name(feature)} "
                        f"but parent is {self.parents[child]}"
                    )

        # 4. Connectivity & acyclic
        visited = set[Feature]()
        stack = [self.root]

        while stack:
            feature = stack.pop()
            if feature in visited:
                raise AssertionError("Cycle detected in feature tree.")
            visited.add(feature)
            stack.extend(self.children[feature])

        if len(visited) != n:
            unreachable = [
                self.feature_name(f_id)
                for f_id in self.features()
                if f_id not in visited
            ]
            raise AssertionError(f"Unreachable features detected: {unreachable}")

        # 5. Leaf cardinalities
        for feature in self.features():
            if self.is_leaf(feature):
                group_instance = self.group_instance_cardinalities[feature]
                group_type = self.group_type_cardinalities[feature]

                if not (group_instance.contains(0) and group_instance.size == 1):
                    raise AssertionError(
                        f"Leaf feature {self.feature_name(feature)} must have group instance cardinality (0,0)."
                    )

                if not (group_type.contains(0) and group_type.size == 1):
                    raise AssertionError(
                        f"Leaf feature {self.feature_name(feature)} must have group type cardinality (0,0)."
                    )

    def compute_instance_bounds(self) -> FeatureList[int]:
        """
        Compute, for each feature, the maximum number of times it can appear
        in an instance-based configuration.

        The bounds are computed as:
            bounds[root] = 1
            bounds[c] = bounds[parent(c)] * max(feature_instance_interval(c))

        Raises a ValueError if any feature has an unbounded
        instance interval.
        """
        bounds: FeatureList[int] = FeatureList([0] * self.n_features)

        for feature in self.traverse_preorder():
            parent = self.parents[feature]

            if parent is None:
                bounds[feature] = 1
                continue

            interval = self.feature_instance_cardinalities[feature]
            interval_bound = interval.max

            if interval_bound is None:
                raise ValueError(
                    f"Cannot compute instance bounds: feature {feature} has an "
                    f"unbounded instance interval ({interval})."
                )

            bounds[feature] = bounds[parent] * interval_bound

        return bounds

    @property
    def n_features(self) -> int:
        return len(self.feature_names)

    def feature_name(self, feature: Feature) -> FeatureName:
        """
        Convert internal feature id → feature name.
        """
        return self.feature_names[feature]

    def feature(self, name: FeatureName) -> Feature:
        """
        Convert feature name → internal feature id.
        """
        return self._encode[name]

    def change_cardinalities(
        self,
        new_feature_instance_cardinalities: FeatureList[CardinalityInterval],
        new_group_instance_cardinalities: FeatureList[CardinalityInterval],
        new_group_type_cardinalities: FeatureList[CardinalityInterval],
    ) -> CFM:
        """
        Create a new CFM from base, by replacing cardinalities.
        """

        builder = CfmBuilder(
            feature_names=[str(name) for name in self.feature_names],
            root=str(self.feature_names[self.root]),
        )

        # tree
        for child_id in self.features():
            parent = self.parents[child_id]
            if parent is not None:
                builder.set_parent(
                    str(self.feature_names[child_id]),
                    str(self.feature_names[parent]),
                )

        # cardinalities
        for f_id in self.features():
            name = str(self.feature_names[f_id])

            builder.set_feature_instance_cardinality(
                name, new_feature_instance_cardinalities[f_id]
            )
            builder.set_group_instance_cardinality(
                name, new_group_instance_cardinalities[f_id]
            )
            builder.set_group_type_cardinality(name, new_group_type_cardinalities[f_id])

        # constraints
        for rc in self.require_constraints:
            builder.add_require_constraint(
                first_feature=str(self.feature_names[rc.first_feature]),
                first_cardinality=rc.first_cardinality,
                second_cardinality=rc.second_cardinality,
                second_feature=str(self.feature_names[rc.second_feature]),
            )

        for ec in self.exclude_constraints:
            builder.add_exclude_constraint(
                first_feature=str(self.feature_names[ec.first_feature]),
                first_cardinality=ec.first_cardinality,
                second_cardinality=ec.second_cardinality,
                second_feature=str(self.feature_names[ec.second_feature]),
            )

        return builder.build()

    @property
    def is_finite(self) -> bool:
        """
        True only if we can *guarantee* the configuration space is finite
        using a simple sufficient condition:
          - all feature-instance cardinalities are bounded

        If this returns False, the configuration space may still be finite.
        """
        return all(
            card.size is not None for card in self.feature_instance_cardinalities
        )

    @property
    def has_cross_tree_constraints(self) -> bool:
        return len(self.require_constraints) > 0 or len(self.exclude_constraints) > 0

    def _tree_equal_by_name(
        self,
        other: CFM,
        self_node: Feature,
        other_node: Feature,
    ) -> bool:
        """
        Recursively compare two trees using feature names,
        ignoring FeatureId encoding.
        """

        # Names must match
        if self.feature_name(self_node) != other.feature_name(other_node):
            return False

        self_children = self.children[self_node]
        other_children = other.children[other_node]

        if len(self_children) != len(other_children):
            return False

        # Build name -> child mapping for other side
        other_by_name = {other.feature_name(c): c for c in other_children}

        # Every self child must exist in other under same name
        for c1 in self_children:
            name = self.feature_name(c1)
            c2 = other_by_name.get(name)
            if c2 is None:
                return False

            # Recurse
            if not self._tree_equal_by_name(other, c1, c2):
                return False

        return True

    def _cards_equal_by_name(
        self,
        other: "CFM",
        self_cards: FeatureTuple[CardinalityInterval],
        other_cards: FeatureTuple[CardinalityInterval],
    ) -> bool:
        for feature in self.features():
            name = self.feature_name(feature)
            other_feature = other.feature(name)

            if self_cards[feature] != other_cards[other_feature]:
                return False

        return True

    @staticmethod
    def _require_key(
        c: RequireConstraint,
        id_to_name: dict[Feature, FeatureName],
    ) -> tuple[FeatureName, CardinalityInterval, CardinalityInterval, FeatureName]:
        return (
            id_to_name[c.first_feature],
            c.first_cardinality,
            c.second_cardinality,
            id_to_name[c.second_feature],
        )

    @staticmethod
    def _exclude_key(
        c: ExcludeConstraint,
        id_to_name: dict[Feature, FeatureName],
    ) -> tuple[FeatureName, CardinalityInterval, CardinalityInterval, FeatureName]:
        return (
            id_to_name[c.first_feature],
            c.first_cardinality,
            c.second_cardinality,
            id_to_name[c.second_feature],
        )

    def __eq__(self, other: object) -> bool:
        if self is other:
            return True
        if not isinstance(other, CFM):
            return NotImplemented

        # Tree isomorphic by feature name
        if not self._tree_equal_by_name(other, self.root, other.root):
            return False

        # Cardinalities by feature name
        if not self._cards_equal_by_name(
            other,
            self.feature_instance_cardinalities,
            other.feature_instance_cardinalities,
        ):
            return False

        if not self._cards_equal_by_name(
            other,
            self.group_instance_cardinalities,
            other.group_instance_cardinalities,
        ):
            return False

        if not self._cards_equal_by_name(
            other,
            self.group_type_cardinalities,
            other.group_type_cardinalities,
        ):
            return False

        # Constraints by feature name
        self_feature_to_name = {
            feature: self.feature_name(feature) for feature in self.features()
        }
        other_feature_to_name = {
            feature: other.feature_name(feature) for feature in other.features()
        }

        self_require = sorted(
            self._require_key(c, self_feature_to_name) for c in self.require_constraints
        )
        other_require = sorted(
            self._require_key(c, other_feature_to_name)
            for c in other.require_constraints
        )

        if self_require != other_require:
            return False

        self_exclude = sorted(
            self._exclude_key(c, self_feature_to_name) for c in self.exclude_constraints
        )
        other_exclude = sorted(
            self._exclude_key(c, other_feature_to_name)
            for c in other.exclude_constraints
        )

        if self_exclude != other_exclude:
            return False

        return True

    def pretty_print(self) -> str:
        lines: list[str] = []

        # -----------------
        # Tree printing
        # -----------------
        def node_label(node: Feature) -> str:
            name = self.feature_name(node)
            fi = self.feature_instance_cardinalities[node]
            gi = self.group_instance_cardinalities[node]
            gt = self.group_type_cardinalities[node]
            return f"{name} (FI={fi}, GT={gt}, GI={gi})"

        def fmt_subtree(node: Feature, prefix: str, is_last: bool) -> None:
            connector = "└── " if is_last else "├── "
            lines.append(f"{prefix}{connector}{node_label(node)}")

            next_prefix = prefix + ("    " if is_last else "│   ")

            children = self.children[node]
            for i, child in enumerate(children):
                fmt_subtree(child, next_prefix, i == len(children) - 1)

        # Root (no connector)
        lines.append(node_label(self.root))

        root_children = self.children[self.root]
        for i, child in enumerate(root_children):
            fmt_subtree(child, "", i == len(root_children) - 1)

        # -----------------
        # Cross-tree constraints
        # -----------------
        if self.has_cross_tree_constraints:
            lines.append("")
            lines.append("Cross-tree constraints:")

            for r in self.require_constraints:
                from_name = self.feature_name(r.first_feature)
                to_name = self.feature_name(r.second_feature)

                lines.append(
                    f"  REQUIRE: {from_name} {r.first_cardinality} "
                    f"-> {to_name} {r.second_cardinality}"
                )

            for e in self.exclude_constraints:
                a = self.feature_name(e.first_feature)
                b = self.feature_name(e.second_feature)

                lines.append(
                    f"  EXCLUDE: {a} {e.first_cardinality} "
                    f"x {b} {e.second_cardinality}"
                )

        return "\n".join(lines)

    def __str__(self) -> str:
        lines: list[str] = []
        lines.append("CFM")
        lines.append("=" * 40)

        # --- Tree ---
        lines.append("Tree:")

        def walk(node: Feature, indent: int = 0) -> None:
            name = self.feature_name(node)
            fi = self.feature_instance_cardinalities[node]
            gi = self.group_instance_cardinalities[node]
            gt = self.group_type_cardinalities[node]

            lines.append("  " * indent + f"- {name} (FI={fi}, GI={gi}, GT={gt})")

            for child in self.children[node]:
                walk(child, indent + 1)

        walk(self.root)
        lines.append("")

        # --- Constraints ---
        if self.require_constraints:
            lines.append("Require constraints:")
            for c in self.require_constraints:
                a = self.feature_name(c.first_feature)
                b = self.feature_name(c.second_feature)
                lines.append(
                    f"  {a} {c.first_cardinality} => {b} {c.second_cardinality}"
                )
        else:
            lines.append("Require constraints: none")

        lines.append("")

        if self.exclude_constraints:
            lines.append("Exclude constraints:")
            for c in self.exclude_constraints:
                a = self.feature_name(c.first_feature)
                b = self.feature_name(c.second_feature)
                lines.append(
                    f"  {a} {c.first_cardinality} x {b} {c.second_cardinality}"
                )
        else:
            lines.append("Exclude constraints: none")

        return "\n".join(lines)

    def _to_native_bytes(self) -> bytes:
        """
        Serialize this CFM into the JSON format expected by the Rust backend.
        """
        # --- feature names ---
        feature_names = [str(name) for name in self.feature_names]
        root = str(self.feature_names[self.root])

        # --- parents: child_name -> parent_name ---
        parents: JSON = {}
        for feature in self.features():
            parent = self.parents[feature]
            if parent is not None:
                child_name = str(self.feature_name(feature))
                parent_name = str(self.feature_name(parent))
                parents[child_name] = parent_name

        # --- cardinalities ---
        def cards_map(cards: FeatureTuple[CardinalityInterval]) -> JSON:
            out: dict[str, Any] = {}
            for feature in self.features():
                name = str(self.feature_name(feature))
                out[name] = _card_to_json(cards[feature])
            return out

        feature_instance_cardinalities = cards_map(self.feature_instance_cardinalities)
        group_instance_cardinalities = cards_map(self.group_instance_cardinalities)
        group_type_cardinalities = cards_map(self.group_type_cardinalities)

        # --- constraints ---
        require_constraints: JSON = [
            {
                "first_feature": str(self.feature_name(c.first_feature)),
                "first_cardinality": _card_to_json(c.first_cardinality),
                "second_cardinality": _card_to_json(c.second_cardinality),
                "second_feature": str(self.feature_name(c.second_feature)),
            }
            for c in self.require_constraints
        ]

        exclude_constraints: JSON = [
            {
                "first_feature": str(self.feature_name(c.first_feature)),
                "first_cardinality": _card_to_json(c.first_cardinality),
                "second_cardinality": _card_to_json(c.second_cardinality),
                "second_feature": str(self.feature_name(c.second_feature)),
            }
            for c in self.exclude_constraints
        ]

        feature_names: JSON = feature_names
        root: JSON = root
        feature_instance_cardinalities: JSON = feature_instance_cardinalities
        group_instance_cardinalities: JSON = group_instance_cardinalities
        group_type_cardinalities: JSON = group_type_cardinalities
        require_constraints: JSON = require_constraints
        exclude_constraints: JSON = exclude_constraints

        payload: JSON = {
            "version": 1,
            "feature_names": feature_names,
            "root": root,
            "parents": parents,
            "feature_instance_cardinalities": feature_instance_cardinalities,
            "group_instance_cardinalities": group_instance_cardinalities,
            "group_type_cardinalities": group_type_cardinalities,
            "require_constraints": require_constraints,
            "exclude_constraints": exclude_constraints,
        }

        return json.dumps(payload).encode("utf-8")

    def to_native(self) -> _cfm_native.CFM:
        """
        Convert this Python CFM into a native Rust-backed CFM.
        Requires the PyO3 module to be imported.
        """
        data = self._to_native_bytes()
        return _cfm_native.CFM.from_bytes(data)


def _card_to_json(card: CardinalityInterval) -> JSON:
    """
    Convert CardinalityInterval into JSON form:
    [[lower, upper_or_none], ...]
    """
    return [[iv.lower, iv.upper] for iv in card]
