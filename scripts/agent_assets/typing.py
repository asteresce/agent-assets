"""TypedDict definitions for the data shapes that cross module boundaries.

These are documentation + IDE hints — runtime behaviour is unchanged.
"""

from __future__ import annotations
from typing import TypedDict


class ImportEntry(TypedDict, total=False):
    name: str
    rename: str
    destination: str
    injections: dict


class CategoryConfig(TypedDict, total=False):
    root: str
    imports: list
    injections: dict


class Config(TypedDict, total=False):
    rules: CategoryConfig
    skills: CategoryConfig
    agents: CategoryConfig


class LockEntry(TypedDict):
    category: str
    name: str
    path: str
    hash: str


class LockFile(TypedDict):
    version: int
    entries: list[LockEntry]