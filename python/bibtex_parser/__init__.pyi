from __future__ import annotations

from pathlib import Path
from typing import Any, TextIO

class BibtexParserError(Exception): ...

class Parser:
    tolerant: bool
    capture_source: bool
    preserve_raw: bool
    expand_values: bool
    latex_to_unicode: bool
    def __init__(
        self,
        tolerant: bool = False,
        capture_source: bool = True,
        preserve_raw: bool = True,
        expand_values: bool = False,
        latex_to_unicode: bool = False,
    ) -> None: ...
    def parse(self, text: str, source: str | None = None) -> Document: ...
    def parse_file(self, path: str) -> Document: ...

class Document:
    status: str
    entries: list[Entry]
    comments: list[Comment]
    preambles: list[Preamble]
    strings: list[StringDefinition]
    diagnostics: list[Diagnostic]
    failed_blocks: list[FailedBlock]
    blocks: list[Block]
    def summary(self) -> dict[str, Any]: ...
    def stats(self) -> dict[str, int]: ...
    def entry(self, key: str) -> Entry | None: ...
    def keys(self) -> list[str]: ...
    def rename_key(self, old: str, new: str) -> bool: ...
    def set_entry_type(self, key: str, entry_type: str) -> bool: ...
    def set_field(self, key: str, name: str, value: Value | str | int) -> bool: ...
    def add_field(self, key: str, name: str, value: Value | str | int) -> bool: ...
    def rename_field(self, key: str, old: str, new: str) -> int: ...
    def remove_field(self, key: str, name: str) -> int: ...
    def remove_export_fields(self, names: list[str]) -> int: ...
    def write(self, config: WriterConfig | None = None) -> str: ...
    def validate(self, level: str = "standard") -> list[ValidationIssue]: ...
    def latex_to_unicode(self) -> None: ...
    def __len__(self) -> int: ...

class Entry:
    key: str
    entry_type: str
    status: str
    fields: list[Field]
    raw: str | None
    source: SourceSpan | None
    def get(self, name: str) -> str | None: ...
    def field(self, name: str) -> Field | None: ...
    def authors(self) -> list[PersonName]: ...
    def editors(self) -> list[PersonName]: ...
    def translators(self) -> list[PersonName]: ...
    def date_parts(self) -> DateParts | None: ...
    def doi(self) -> str | None: ...
    def resource_fields(self) -> list[ResourceField]: ...

class Field:
    name: str
    value: Value
    raw: str | None
    raw_value: str | None
    expanded: str | None
    source: SourceSpan | None
    value_source: SourceSpan | None
    def plain_text(self) -> str: ...
    def lossy_text(self) -> str: ...
    def unicode_text(self) -> str: ...

class Value:
    kind: str
    text: str | None
    number_value: int | None
    parts: list[Value]
    @classmethod
    def literal(cls, text: str) -> Value: ...
    @classmethod
    def number(cls, number: int) -> Value: ...
    @classmethod
    def variable(cls, name: str) -> Value: ...
    @classmethod
    def concat(cls, parts: list[Value]) -> Value: ...
    @classmethod
    def from_plain_string(cls, text: str) -> Value: ...
    def to_plain_string(self) -> str: ...
    def to_lossy_string(self) -> str: ...
    def to_bibtex_source(self) -> str: ...
    def to_unicode_string(self) -> str: ...

class Diagnostic:
    severity: str
    code: str
    message: str
    target: str
    source: SourceSpan | None
    snippet: str | None

class SourceSpan:
    source: int | None
    byte_start: int
    byte_end: int
    line: int
    column: int
    end_line: int
    end_column: int

class Comment:
    text: str
    raw: str | None
    source: SourceSpan | None

class Preamble:
    value: Value
    raw: str | None
    source: SourceSpan | None

class StringDefinition:
    name: str
    value: Value
    raw: str | None
    source: SourceSpan | None

class FailedBlock:
    raw: str
    error: str
    source: SourceSpan | None
    diagnostics: list[Diagnostic]

class Block:
    kind: str
    index: int

class WriterConfig:
    indent: str
    align_values: bool
    max_line_length: int
    sort_entries: bool
    sort_fields: bool
    preserve_raw: bool
    trailing_comma: bool
    entry_separator: str
    def __init__(
        self,
        indent: str = "  ",
        align_values: bool = False,
        max_line_length: int = 80,
        sort_entries: bool = False,
        sort_fields: bool = False,
        preserve_raw: bool = True,
        trailing_comma: bool = False,
        entry_separator: str = "\n",
    ) -> None: ...

class ValidationIssue:
    entry_index: int
    key: str
    field: str | None
    severity: str
    message: str

class PersonName:
    raw: str
    given: list[str]
    family: list[str]
    prefix: list[str]
    suffix: list[str]
    literal: str | None
    def display_name(self) -> str: ...

class DateParts:
    year: int
    month: int | None
    day: int | None

class ResourceField:
    kind: str
    field_name: str
    value: str
    normalized: str | None

def parse(
    text: str,
    *,
    tolerant: bool = False,
    capture_source: bool = True,
    preserve_raw: bool = True,
    expand_values: bool = False,
    latex_to_unicode: bool = False,
    source: str | None = None,
) -> Document: ...
def loads(text: str, **kwargs: Any) -> Document: ...
def load(file_obj: TextIO, **kwargs: Any) -> Document: ...
def dumps(document: Document, config: WriterConfig | None = None) -> str: ...
def dump(document: Document, file_obj: TextIO, config: WriterConfig | None = None) -> None: ...
def parse_path(path: str | Path, **kwargs: Any) -> Document: ...
def parse_text(
    text: str,
    tolerant: bool = False,
    capture_source: bool = True,
    preserve_raw: bool = True,
    expand_values: bool = False,
    latex_to_unicode: bool = False,
    source: str | None = None,
) -> Document: ...
def parse_file(
    path: str,
    tolerant: bool = False,
    capture_source: bool = True,
    preserve_raw: bool = True,
    expand_values: bool = False,
    latex_to_unicode: bool = False,
) -> Document: ...
def write(document: Document, config: WriterConfig | None = None) -> str: ...
def normalize_doi(input: str) -> str | None: ...
def parse_names(input: str) -> list[PersonName]: ...
def parse_date(input: str) -> DateParts: ...
def latex_to_unicode(input: str) -> str: ...
