"""Where the benchmark's outside data is. None of it is in the repository, and nothing is
guessed: each path comes from its environment variable, else from the same key in
~/.config/boardroom/config.toml, else the tool stops and says which one is missing.

    BOARDROOM_LAB      bench_lab      the lab directory: prepared boards, runs, review pages
    BOARDROOM_CORPUS   bench_corpus   the PCBench checkout (many boards are not redistributable:
                                      it never enters this repository)
    BOARDROOM_ROUTER   bench_router   the router binary under test or the pinned baseline, with
                                      kicad-text beside it (the old router is never built here)
"""
import os
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
CONFIG = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "boardroom" / "config.toml"


def _configured(env, key, what):
    value = os.environ.get(env)
    if not value and CONFIG.is_file():
        value = tomllib.loads(CONFIG.read_text()).get(key)
    if not value:
        sys.exit(f"bench: {what} is not set. Set ${env}, or {key} = \"...\" in {CONFIG}.")
    return Path(value).expanduser()


def lab():
    return _configured("BOARDROOM_LAB", "bench_lab", "the lab directory")


def corpus():
    return _configured("BOARDROOM_CORPUS", "bench_corpus", "the PCBench checkout")


def router():
    return _configured("BOARDROOM_ROUTER", "bench_router", "the router binary")
