from pathlib import Path


def get_test_models_dir() -> Path:
    here = Path(__file__).resolve()
    return here.parent.parent / "models"


TEST_MODEL_PATH = get_test_models_dir()
