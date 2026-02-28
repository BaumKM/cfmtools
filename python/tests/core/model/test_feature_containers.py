import pytest

from cfmtools.core.cfm import Feature, FeatureList, FeatureTuple


def test_feature_list_all_methods_and_equality():
    a = FeatureList([1, 2, 3])
    b = FeatureList([1, 2, 3])
    c = FeatureList([1, 99, 3])

    # __len__
    assert len(a) == 3

    # __getitem__
    assert a[Feature(0)] == 1
    assert a[Feature(2)] == 3

    # __setitem__
    a[Feature(1)] = 2
    assert a[Feature(1)] == 2

    # __iter__
    assert list(a) == [1, 2, 3]

    # as_feature_tuple
    tup = a.as_feature_tuple()
    assert isinstance(tup, FeatureTuple)
    assert list(tup) == [1, 2, 3]

    # equality (same contents)
    assert a == b
    assert not (a != b)

    # inequality (different contents)
    assert a != c

    # comparison with unrelated type
    assert (a == [1, 2, 3]) is False


def test_feature_tuple_all_methods_and_equality():
    a = FeatureTuple(["x", "y", "z"])
    b = FeatureTuple(["x", "y", "z"])
    c = FeatureTuple(["x", "q", "z"])

    # __len__
    assert len(a) == 3

    # __getitem__
    assert a[Feature(0)] == "x"
    assert a[Feature(1)] == "y"

    # __iter__
    assert list(a) == ["x", "y", "z"]

    # equality (same contents)
    assert a == b
    assert not (a != b)

    # inequality (different contents)
    assert a != c

    # comparison with unrelated type
    assert (a == ("x", "y", "z")) is False

    # immutability
    with pytest.raises(TypeError):
        a[Feature(0)] = "changed"  # type: ignore
