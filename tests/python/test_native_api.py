from __future__ import annotations

import io

import citerra


def test_parse_inspect_mutate_write_parse_counts() -> None:
    text = """% kept
@string{venue = "ToolConf"}
@article{paper,
  author = "Jos\\'e Garc\\'ia and {Research Group}",
  title = "Example Paper",
  journal = venue,
  year = 2026,
  doi = "https://doi.org/10.1000/XYZ."
}
@preamble{"prefix"}
"""

    document = citerra.parse(text, expand_values=True)

    assert document.status == "ok"
    assert len(document) == 1
    assert document.stats() == {
        "entries": 1,
        "comments": 1,
        "preambles": 1,
        "strings": 1,
        "failed_blocks": 0,
        "diagnostics": 0,
    }

    entry = document.entry("paper")
    assert entry is not None
    assert entry.get("journal") == "ToolConf"
    assert entry.doi() == "10.1000/xyz"
    assert entry.authors()[1].literal == "Research Group"
    assert entry.date_parts().year == 2026
    assert entry.field("title").raw_value == '"Example Paper"'
    assert entry.source.line == 3

    assert document.rename_key("paper", "paper2")
    assert document.set_field("paper2", "title", "Updated Paper")
    assert document.add_field("paper2", "note", citerra.Value.literal("accepted"))
    output = document.write()

    reparsed = citerra.parse(output)
    assert reparsed.keys() == ["paper2"]
    assert reparsed.entry("paper2").get("title") == "Updated Paper"
    assert reparsed.stats()["comments"] == 1
    assert reparsed.stats()["preambles"] == 1
    assert reparsed.stats()["strings"] == 1


def test_tolerant_parse_exposes_diagnostics_and_failed_blocks() -> None:
    text = """@article{bad title = "missing comma"}
@book{ok, title = "Recovered"}"""

    document = citerra.parse(text, tolerant=True)

    assert document.status == "partial"
    assert document.keys() == ["ok"]
    assert len(document.failed_blocks) == 1
    assert document.diagnostics[0].code == "missing-field-separator"
    assert document.diagnostics[0].source.line == 1


def test_file_like_loading_and_writer_config() -> None:
    source = io.StringIO('@article{a, title = "A"}')
    source.name = "memory.bib"

    document = citerra.load(source)
    output = citerra.dumps(
        document,
        citerra.WriterConfig(preserve_raw=False, trailing_comma=True),
    )

    assert "@article{a," in output
    assert "title = {A}," in output
    assert citerra.parse(output).keys() == ["a"]


def test_semantic_text_deindents_wrapped_lines_but_keeps_raw_value() -> None:
    document = citerra.parse(
        """@article{wrapped,
  title = {First line
                  Second line
                  Third line}
}"""
    )

    entry = document.entry("wrapped")
    assert entry.get("title") == "First line\nSecond line\nThird line"
    assert (
        entry.field("title").raw_value
        == "{First line\n                  Second line\n                  Third line}"
    )


def test_unmodified_document_writes_preserved_source_text() -> None:
    text = """@article{paper,
  title   = "Exact Spacing",
  year = 2026,
}
% trailing comment
"""

    document = citerra.parse(text)

    assert document.entry("paper").raw == text.split("\n% trailing comment")[0]
    assert document.entry("paper").field("title").raw == 'title   = "Exact Spacing",'
    assert citerra.dumps(document) == text


def test_structured_parse_can_skip_source_capture_and_project_records() -> None:
    text = """@string{venue = "VLDB"}
@article{paper, author = "Jos\\'e", title = venue, month = jan, year = 2026}"""

    document = citerra.parse(text, capture_source=False, preserve_raw=False)
    entry = document.entry("paper")

    assert entry.source is None
    assert entry.field("title").raw_value is None
    assert entry.get("title") == "venue"
    assert entry.get("month") == "jan"
    assert document.to_dicts() == [
        {
            "ENTRYTYPE": "article",
            "ID": "paper",
            "author": "Jos\\'e",
            "title": "venue",
            "month": "jan",
            "year": "2026",
        }
    ]
    assert citerra.document_to_dicts(document) == document.to_dicts()

    expanded = citerra.parse(
        text,
        capture_source=False,
        preserve_raw=False,
        expand_values=True,
    )
    assert expanded.entry("paper").get("title") == "VLDB"
    assert expanded.entry("paper").get("month") == "January"

    expanded_unicode = citerra.parse(
        text,
        capture_source=False,
        preserve_raw=False,
        expand_values=True,
        latex_to_unicode=True,
    )
    assert expanded_unicode.entry("paper").get("author") == "José"
    assert expanded_unicode.entry("paper").get("title") == "VLDB"
    assert expanded_unicode.entry("paper").get("month") == "January"


def test_plain_record_helpers_cover_rebuild_and_selected_entry_workflows() -> None:
    document = citerra.parse(
        """@comment{keep}
@article{b, title = {Second}, year = {2024}}
@article{a, title = {First}, year = {2023}}"""
    )

    records = citerra.document_to_dicts(document)
    assert records[0]["ENTRYTYPE"] == "article"
    assert records[0]["ID"] == "b"
    assert records[0]["title"] == "Second"

    rebuilt = citerra.document_from_entries(
        records,
        comments=["keep"],
        field_order=["year", "title"],
        sort_by=["ID"],
        trailing_comma=True,
    )
    output = rebuilt.write()

    assert rebuilt.keys() == ["a", "b"]
    assert "@comment{keep}" in output
    assert "year = {2023}," in output
    assert output.index("@article{a,") < output.index("@article{b,")
    assert output.index("year = {2023}") < output.index("title = {First}")
    assert document.write_selected(["a"]).strip().startswith("@article{a,")
    assert "@article{b," not in document.write_selected(["a"])

    latex_output = citerra.write_entries(
        [{"ENTRYTYPE": "article", "ID": "latex", "title": "Jos\\'e and \\alpha"}]
    )
    assert "Jos\\'e and \\alpha" in latex_output
    assert citerra.parse(latex_output).entry("latex").get("title") == "Jos\\'e and \\alpha"


def test_write_entries_serializes_latex_rich_plain_strings_parse_clean() -> None:
    output = citerra.write_entries(
        [
            {
                "ENTRYTYPE": "article",
                "ID": "latex-rich",
                "title": r"Learning with $\mathrm{SO}(3)$ and {Protected Words}",
                "abstract": (
                    "First line\n"
                    r"Second line with \mathrm{X}, https://example.test/a?b={c}, "
                    r"and escaped \{braces\}"
                ),
                "note": 'stray closing } and "quoted" text',
            }
        ],
        trailing_comma=True,
    )

    reparsed = citerra.parse(output, tolerant=True)

    assert reparsed.stats()["entries"] == 1
    assert reparsed.stats()["diagnostics"] == 0
    entry = reparsed.entry("latex-rich")
    assert entry.get("title") == r"Learning with $\mathrm{SO}(3)$ and {Protected Words}"
    assert r"\mathrm{X}" in entry.get("abstract")
    assert entry.get("note") == r"stray closing } and \"quoted\" text"


def test_document_record_overlay_preserves_blocks_and_unchanged_latex_source() -> None:
    text = r"""@string{venue = "ICLR"}
% keep this comment
@preamble{"prefix"}
@article{paper,
  title = {Learning with $\mathrm{SO}(3)$ and {Protected Words}},
  abstract = {First line
    Second line with \mathrm{X} and {nested text}},
  booktitle = venue
}
"""
    document = citerra.parse(text)
    records = document.to_dicts()
    records[0]["pdf"] = "paper.pdf"
    records[0]["eprint"] = citerra.Value.from_bibtex_source("arxiv_id # {v2}")

    summary = document.update_from_dicts(records)
    output = document.write()
    reparsed = citerra.parse(output, tolerant=True)

    assert summary["matched_entries"] == 1
    assert summary["added_fields"] == 2
    assert summary["unchanged_fields"] >= 3
    assert "title = {Learning with $\\mathrm{SO}(3)$ and {Protected Words}}" in output
    assert "abstract = {First line" in output
    assert "@string{venue = \"ICLR\"}" in output
    assert "% keep this comment" in output
    assert "@preamble{\"prefix\"}" in output
    assert reparsed.stats()["diagnostics"] == 0
    assert reparsed.stats()["entries"] == 1
    assert reparsed.stats()["strings"] == 1
    assert reparsed.stats()["preambles"] == 1
    assert reparsed.stats()["comments"] == 1
    assert reparsed.entry("paper").get("pdf") == "paper.pdf"
    assert reparsed.entry("paper").field("eprint").value.kind == "concat"


def test_value_mode_records_preserve_macros_and_support_new_entries() -> None:
    document = citerra.parse(
        """@string{venue = "VLDB"}
@article{paper, title = venue # " 2026", year = 2026}"""
    )

    records = document.to_dicts(value_mode="value")
    assert records[0]["title"].kind == "concat"
    assert records[0]["title"].to_plain_string() == "venue 2026"
    records.append(
        {
            "ENTRYTYPE": "misc",
            "ID": "new",
            "title": citerra.Value.from_bibtex_source("venue # { extended}"),
        }
    )

    summary = document.update_from_dicts(records, create_missing=True)
    output = document.write()
    reparsed = citerra.parse(output, tolerant=True)

    assert summary["added_entries"] == 1
    assert reparsed.stats()["diagnostics"] == 0
    assert reparsed.keys() == ["paper", "new"]
    assert reparsed.entry("new").field("title").value.kind == "concat"


def test_update_from_dicts_partial_record_keeps_existing_entry_type() -> None:
    document = citerra.parse("@book{paper, title = {Before}}")

    summary = document.update_from_dicts([{"ID": "paper", "title": "After"}])
    output = document.write()
    reparsed = citerra.parse(output, tolerant=True)

    assert summary["changed_entry_types"] == 0
    assert reparsed.stats()["diagnostics"] == 0
    assert reparsed.entry("paper").entry_type == "book"
    assert reparsed.entry("paper").get("title") == "After"


def test_helpers_are_native_and_typed() -> None:
    assert citerra.normalize_doi("https://doi.org/10.1000/XYZ.") == "10.1000/xyz"
    assert citerra.parse_date("2026-05-13").month == 5
    assert citerra.parse_names("Jane Doe and {ACME Lab}")[1].literal == "ACME Lab"
    assert citerra.latex_to_unicode("Jos\\'e") == "José"
