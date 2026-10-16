# Short link service backend
import tomllib
from pathlib import Path

with (Path(__file__).resolve().parents[1] / "pyproject.toml").open("rb") as _project:
    __version__ = tomllib.load(_project)["project"]["version"]
