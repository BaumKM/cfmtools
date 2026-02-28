from cfmtools.plugins.load.uvl import UVLFeatureModelLoader
from tests.data.target.uvl.fm.valid.ctc_1 import ctc_1_cfm, CTC_1_PATH
from tests.data.target.uvl.fm.valid.ctc_2 import ctc_2_cfm, CTC_2_PATH
from tests.data.target.uvl.fm.valid.ctc_3 import ctc_3_cfm, CTC_3_PATH
from tests.data.target.uvl.fm.valid.ctc_4 import ctc_4_cfm, CTC_4_PATH
from tests.data.target.uvl.fm.valid.ctc_5 import ctc_5_cfm, CTC_5_PATH
from tests.data.target.uvl.fm.valid.ctc_6 import ctc_6_cfm, CTC_6_PATH
from tests.data.target.uvl.fm.valid.ctc_7 import ctc_7_cfm, CTC_7_PATH
from tests.data.target.uvl.fm.valid.sandwich import (
    SANDWICH_PATH,
    sandwich_cfm,
)
from tests.data.target.uvl.fm.valid.table import TABLE_PATH, table_cfm


def test_sandwich():
    loader = UVLFeatureModelLoader(path=SANDWICH_PATH)  # type: ignore
    model = loader.load()
    assert model == sandwich_cfm()


def test_table():
    loader = UVLFeatureModelLoader(path=TABLE_PATH)  # type: ignore
    model = loader.load()
    assert model == table_cfm()


def test_ctc_1():
    loader = UVLFeatureModelLoader(path=CTC_1_PATH)  # type: ignore
    model = loader.load()
    assert model == ctc_1_cfm()


def test_ctc_2():
    loader = UVLFeatureModelLoader(path=CTC_2_PATH)  # type: ignore
    model = loader.load()
    assert model == ctc_2_cfm()


def test_ctc_3():
    loader = UVLFeatureModelLoader(path=CTC_3_PATH)  # type: ignore
    model = loader.load()
    assert model == ctc_3_cfm()


def test_ctc_4():
    loader = UVLFeatureModelLoader(path=CTC_4_PATH)  # type: ignore
    model = loader.load()
    assert model == ctc_4_cfm()


def test_ctc_5():
    loader = UVLFeatureModelLoader(path=CTC_5_PATH)  # type: ignore
    model = loader.load()
    assert model == ctc_5_cfm()


def test_ctc_6():
    loader = UVLFeatureModelLoader(path=CTC_6_PATH)  # type: ignore
    model = loader.load()
    assert model == ctc_6_cfm()


def test_ctc_7():
    loader = UVLFeatureModelLoader(path=CTC_7_PATH)  # type: ignore
    model = loader.load()
    assert model == ctc_7_cfm()
