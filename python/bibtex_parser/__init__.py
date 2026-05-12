"""Native Python API for the Rust ``bibtex-parser`` crate."""

from __future__ import annotations

from pathlib import Path
from typing import Any, TextIO

from ._native import (
    BibtexParserError,
    Block,
    Comment,
    DateParts,
    Diagnostic,
    Document,
    Entry,
    FailedBlock,
    Field,
    Parser,
    PersonName,
    Preamble,
    ResourceField,
    SourceSpan,
    StringDefinition,
    ValidationIssue,
    Value,
    WriterConfig,
    latex_to_unicode,
    normalize_doi,
    parse_date,
    parse_file,
    parse_names,
    parse_text,
    write,
)

__all__ = [
    "BibtexParserError",
    "Block",
    "Comment",
    "DateParts",
    "Diagnostic",
    "Document",
    "Entry",
    "FailedBlock",
    "Field",
    "Parser",
    "PersonName",
    "Preamble",
    "ResourceField",
    "SourceSpan",
    "StringDefinition",
    "ValidationIssue",
    "Value",
    "WriterConfig",
    "dump",
    "dumps",
    "latex_to_unicode",
    "load",
    "loads",
    "normalize_doi",
    "parse",
    "parse_date",
    "parse_file",
    "parse_names",
    "parse_path",
    "parse_text",
    "write",
]


def parse(
    text: str,
    *,
    tolerant: bool = False,
    capture_source: bool = True,
    preserve_raw: bool = True,
    expand_values: bool = False,
    latex_to_unicode: bool = False,
    source: str | None = None,
) -> Document:
    return parse_text(
        text,
        tolerant=tolerant,
        capture_source=capture_source,
        preserve_raw=preserve_raw,
        expand_values=expand_values,
        latex_to_unicode=latex_to_unicode,
        source=source,
    )


def loads(text: str, **kwargs: Any) -> Document:
    return parse(text, **kwargs)


def load(file_obj: TextIO, **kwargs: Any) -> Document:
    text = file_obj.read()
    source = kwargs.pop("source", getattr(file_obj, "name", None))
    return parse(text, source=source, **kwargs)


def dumps(document: Document, config: WriterConfig | None = None) -> str:
    return write(document, config=config)


def dump(document: Document, file_obj: TextIO, config: WriterConfig | None = None) -> None:
    file_obj.write(dumps(document, config=config))


def parse_path(path: str | Path, **kwargs: Any) -> Document:
    return parse_file(str(path), **kwargs)
