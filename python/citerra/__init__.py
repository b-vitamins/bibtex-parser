"""BibTeX parser for Python."""

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
    _document_to_dicts,
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
    "document_from_entries",
    "document_to_dicts",
    "entry_to_dict",
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
    "write_entries",
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


def entry_to_dict(entry: Entry) -> dict[str, str]:
    """Project a native entry into a plain record for application code."""
    record: dict[str, str] = {"ENTRYTYPE": entry.entry_type, "ID": entry.key}
    for field in entry.fields:
        record[field.name] = field.plain_text()
    return record


def document_to_dicts(document: Document) -> list[dict[str, str]]:
    """Project all entries into plain records in document order."""
    return _document_to_dicts(document)


def document_from_entries(
    entries: list[dict[str, Any]],
    *,
    comments: list[str] | None = None,
    preambles: list[str] | None = None,
    strings: dict[str, Any] | list[tuple[str, Any]] | None = None,
    field_order: list[str] | tuple[str, ...] | None = None,
    sort_by: list[str] | tuple[str, ...] | None = None,
    reverse: bool = False,
    trailing_comma: bool = False,
    entry_separator: str = "\n\n",
) -> Document:
    """Build a native document from plain entry records."""
    return parse(
        write_entries(
            entries,
            comments=comments,
            preambles=preambles,
            strings=strings,
            field_order=field_order,
            sort_by=sort_by,
            reverse=reverse,
            trailing_comma=trailing_comma,
            entry_separator=entry_separator,
        ),
        preserve_raw=True,
    )


def write_entries(
    entries: list[dict[str, Any]],
    *,
    comments: list[str] | None = None,
    preambles: list[str] | None = None,
    strings: dict[str, Any] | list[tuple[str, Any]] | None = None,
    field_order: list[str] | tuple[str, ...] | None = None,
    sort_by: list[str] | tuple[str, ...] | None = None,
    reverse: bool = False,
    trailing_comma: bool = False,
    entry_separator: str = "\n\n",
) -> str:
    """Serialize plain entry records into BibTeX source."""
    rendered: list[str] = []
    for comment in comments or []:
        text = str(comment)
        stripped = text.lstrip()
        if stripped.startswith("%") or stripped.startswith("@"):
            rendered.append(text)
        else:
            rendered.append(f"@comment{{{text}}}")

    for preamble in preambles or []:
        rendered.append(f"@preamble{{{_value_source(preamble)}}}")

    if strings:
        items = strings.items() if isinstance(strings, dict) else strings
        for name, value in items:
            rendered.append(f"@string{{{name} = {_value_source(value)}}}")

    ordered_entries = list(entries)
    if sort_by:
        keys = tuple(sort_by)
        ordered_entries.sort(
            key=lambda entry: tuple(str(entry.get(key, "")) for key in keys),
            reverse=reverse,
        )

    rendered.extend(
        _render_entry(entry, field_order=field_order, trailing_comma=trailing_comma)
        for entry in ordered_entries
    )
    return entry_separator.join(block for block in rendered if block)


def _render_entry(
    entry: dict[str, Any],
    *,
    field_order: list[str] | tuple[str, ...] | None,
    trailing_comma: bool,
) -> str:
    entry_type = str(entry.get("ENTRYTYPE", "article")).strip() or "article"
    key = str(entry.get("ID", "")).strip()
    fields = _ordered_fields(entry, field_order)
    lines = [f"@{entry_type}{{{key},"]
    for index, field in enumerate(fields):
        comma = "," if index < len(fields) - 1 or trailing_comma else ""
        lines.append(f"  {field} = {_value_source(entry[field])}{comma}")
    lines.append("}")
    return "\n".join(lines)


def _ordered_fields(
    entry: dict[str, Any],
    field_order: list[str] | tuple[str, ...] | None,
) -> list[str]:
    out: list[str] = []
    seen = {"ENTRYTYPE", "ID"}
    for field in field_order or ():
        if field in entry and field not in seen:
            out.append(field)
            seen.add(field)
    for field in entry:
        if field not in seen:
            out.append(field)
            seen.add(field)
    return out


def _value_source(value: Any) -> str:
    if isinstance(value, Value):
        return value.to_bibtex_source()
    text = str(value)
    return "{" + text.replace("}", "\\}") + "}"
