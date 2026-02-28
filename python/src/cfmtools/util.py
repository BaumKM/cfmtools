from typing import Mapping, TypeAlias

JSON: TypeAlias = str | int | float | list["JSON"] | Mapping[str, "JSON"] | None
