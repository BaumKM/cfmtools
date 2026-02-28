from pathlib import Path

from pytest import MonkeyPatch


def mock_path(monkeypatch: MonkeyPatch, *, text: str, exists: bool = True):
    """
    Mocks a Path
    """

    def fake_exists(self) -> bool:  # type: ignore
        return exists

    def fake_read_text(self, encoding=None) -> str:  # type: ignore
        return text

    monkeypatch.setattr(Path, "exists", fake_exists)  # type: ignore
    monkeypatch.setattr(Path, "read_text", fake_read_text)  # type: ignore
