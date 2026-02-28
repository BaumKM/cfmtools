from pathlib import Path
import pytest
from cfmtools.plugins.load.uvl import (
    UVLFeatureModelLoader,
    UVLUnsupportedError,
)
from tests.data.target.uvl.fm.invalid.ctc_invalid import (
    CTC_1_PATH,
    CTC_2_PATH,
    CTC_3_PATH,
    CTC_4_PATH,
    CTC_5_PATH,
    CTC_6_PATH,
    CTC_7_PATH,
)

INVALID_CASES = [
    CTC_1_PATH,
    CTC_2_PATH,
    CTC_3_PATH,
    CTC_4_PATH,
    CTC_5_PATH,
    CTC_6_PATH,
    CTC_7_PATH,
]


@pytest.mark.parametrize("path", INVALID_CASES)
def test_invalid_ctc_models(path: Path):
    loader = UVLFeatureModelLoader(path=path)  # type: ignore

    with pytest.raises((UVLUnsupportedError)):
        loader.load()
