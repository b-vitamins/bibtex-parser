from __future__ import annotations

import io

import bibtex_parser


def test_parse_inspect_mutate_write_parse_counts() -> None:
    text = """% kept
@string{venue = "ToolConf"}
@article{paper,
  author = "Jos\\'e Garc\\'ia and {Research Group}",
  title = "Fast BibTeX",
  journal = venue,
  year = 2026,
  doi = "https://doi.org/10.1000/XYZ."
}
@preamble{"prefix"}
"""

    document = bibtex_parser.parse(text, expand_values=True)

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
    assert entry.field("title").raw_value == '"Fast BibTeX"'
    assert entry.source.line == 3

    assert document.rename_key("paper", "paper2")
    assert document.set_field("paper2", "title", "Faster BibTeX")
    assert document.add_field("paper2", "note", bibtex_parser.Value.literal("accepted"))
    output = document.write()

    reparsed = bibtex_parser.parse(output)
    assert reparsed.keys() == ["paper2"]
    assert reparsed.entry("paper2").get("title") == "Faster BibTeX"
    assert reparsed.stats()["comments"] == 1
    assert reparsed.stats()["preambles"] == 1
    assert reparsed.stats()["strings"] == 1


def test_tolerant_parse_exposes_diagnostics_and_failed_blocks() -> None:
    text = """@article{bad title = "missing comma"}
@book{ok, title = "Recovered"}"""

    document = bibtex_parser.parse(text, tolerant=True)

    assert document.status == "partial"
    assert document.keys() == ["ok"]
    assert len(document.failed_blocks) == 1
    assert document.diagnostics[0].code == "missing-field-separator"
    assert document.diagnostics[0].source.line == 1


def test_file_like_loading_and_writer_config() -> None:
    source = io.StringIO('@article{a, title = "A"}')
    source.name = "memory.bib"

    document = bibtex_parser.load(source)
    output = bibtex_parser.dumps(
        document,
        bibtex_parser.WriterConfig(preserve_raw=False, trailing_comma=True),
    )

    assert "@article{a," in output
    assert "title = {A}," in output
    assert bibtex_parser.parse(output).keys() == ["a"]


def test_helpers_are_native_and_typed() -> None:
    assert bibtex_parser.normalize_doi("https://doi.org/10.1000/XYZ.") == "10.1000/xyz"
    assert bibtex_parser.parse_date("2026-05-13").month == 5
    assert bibtex_parser.parse_names("Jane Doe and {ACME Lab}")[1].literal == "ACME Lab"
    assert bibtex_parser.latex_to_unicode("Jos\\'e") == "José"
