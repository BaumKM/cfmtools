import itertools
import pytest

from cfmtools.core.cfm import CardinalityInterval, SimpleCardinalityInterval

# ----------------------
# SimpleCardinalityInterval
# ----------------------


def test_simple_interval_contains_and_size():
    iv = SimpleCardinalityInterval(2, 4)

    assert iv.contains(2)
    assert iv.contains(3)
    assert iv.contains(4)
    assert not iv.contains(1)
    assert not iv.contains(5)

    assert iv.size == 3


def test_simple_interval_unbounded():
    iv = SimpleCardinalityInterval(3, None)

    assert iv.contains(100)
    assert iv.size is None


def test_simple_interval_invalid_bounds():
    with pytest.raises(ValueError):
        SimpleCardinalityInterval(-1, 3)

    with pytest.raises(ValueError):
        SimpleCardinalityInterval(5, 2)


# ----------------------
# CardinalityInterval normalization
# ----------------------


def test_cardinality_interval_merging_and_sorting():
    iv1 = SimpleCardinalityInterval(5, 7)
    iv2 = SimpleCardinalityInterval(1, 3)
    iv3 = SimpleCardinalityInterval(4, 4)

    ci = CardinalityInterval([iv1, iv2, iv3])

    # Should merge into [1, 7]
    assert ci.min == 1
    assert ci.max == 7
    assert ci.size == 7


@pytest.mark.parametrize(
    "intervals,expected_normalized",
    [
        ([(5, None), (5, None)], [(5, None)]),
        ([(10, None), (3, None)], [(3, None)]),
        ([(1, 4), (5, None)], [(1, None)]),
        ([(1, 10), (3, None)], [(1, None)]),
        ([(20, None), (1, 3), (5, None), (4, 4)], [(1, None)]),
    ],
    ids=[
        "duplicate-unbounded",
        "two-unbounded-different-lower",
        "adjacent-to-unbounded",
        "overlap-with-unbounded",
        "multiple-mixed",
    ],
)
def test_cardinality_interval_merge_multiple_unbounded(
    intervals: list[tuple[int, int]],
    expected_normalized: list[SimpleCardinalityInterval],
):
    simple_intervals = [SimpleCardinalityInterval(lo, hi) for lo, hi in intervals]
    ci = CardinalityInterval(simple_intervals)

    normalized = [(iv.lower, iv.upper) for iv in ci]
    assert normalized == expected_normalized


def test_cardinality_interval_contains():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(1, 3),
            SimpleCardinalityInterval(10, 12),
        ]
    )

    assert ci.contains(1)
    assert ci.contains(2)
    assert ci.contains(11)
    assert not ci.contains(5)
    assert not ci.contains(20)


def test_cardinality_interval_contains_compound_unbounded():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(1, 3),
            SimpleCardinalityInterval(10, None),
        ]
    )

    assert ci.contains(1)
    assert ci.contains(3)
    assert not ci.contains(5)  # gap
    assert ci.contains(10)
    assert ci.contains(1_000_000)


def test_cardinality_interval_unbounded_size():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(1, 3),
            SimpleCardinalityInterval(10, None),
        ]
    )

    assert ci.size is None
    assert ci.max is None
    assert ci.non_convex_bound == 10


def test_cardinality_interval_values_finite():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(1, 2),
            SimpleCardinalityInterval(5, 6),
        ]
    )

    assert list(ci.values()) == [1, 2, 5, 6]


def test_cardinality_interval_bound():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(1, 5),
            SimpleCardinalityInterval(10, None),
        ]
    )

    bounded = ci.bound(3)

    assert list(bounded.values()) == [1, 2, 3]


def test_non_convex_bound_bounded():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(1, 3),
            SimpleCardinalityInterval(10, 12),
        ]
    )

    # Fully bounded -> maximum finite value
    assert ci.non_convex_bound == 12


def test_non_convex_bound_unbounded():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(1, 3),
            SimpleCardinalityInterval(10, None),
        ]
    )

    # Unbounded tail -> lower bound of the infinite interval
    assert ci.non_convex_bound == 10


def test_cardinality_interval_empty_defaults_to_zero():
    ci = CardinalityInterval([])

    # Default interval is [0, 0]
    assert ci.contains(0)
    assert not ci.contains(1)
    assert ci.size == 1


def test_cardinality_interval_values_infinite_prefix():
    ci = CardinalityInterval(
        [
            SimpleCardinalityInterval(3, None),
        ]
    )

    first_values = list(itertools.islice(ci.values(), 5))

    assert first_values == [3, 4, 5, 6, 7]
