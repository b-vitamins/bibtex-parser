use bibtex_parser::Database;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn generate_bibtex(n_entries: usize) -> String {
    let mut bib = String::with_capacity(n_entries * 200);

    // Add some string definitions
    bib.push_str(
        r#"@string{ieee = "IEEE Transactions"}
@string{acm = "ACM Computing Surveys"}

"#,
    );

    // Generate entries
    for i in 0..n_entries {
        let entry = format!(
            r#"@article{{entry{},
    author = "Author {} and Coauthor {}",
    title = "Title of Paper Number {}",
    journal = ieee,
    year = {},
    volume = {},
    pages = "{}-{}"
}}

"#,
            i,
            i,
            i,
            i,
            2000 + (i % 25),
            i % 50,
            i * 10,
            i * 10 + 9
        );
        bib.push_str(&entry);
    }

    bib
}

fn bench_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing");

    for size in [10, 100, 1000].iter() {
        let input = generate_bibtex(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| {
                let db = Database::parse(black_box(input)).unwrap();
                black_box(db);
            });
        });
    }

    group.finish();
}

fn bench_real_files(c: &mut Criterion) {
    let simple = include_str!("../tests/fixtures/simple.bib");
    let complex = include_str!("../tests/fixtures/complex.bib");

    c.bench_function("parse_simple", |b| {
        b.iter(|| {
            let db = Database::parse(black_box(simple)).unwrap();
            black_box(db);
        });
    });

    c.bench_function("parse_complex", |b| {
        b.iter(|| {
            let db = Database::parse(black_box(complex)).unwrap();
            black_box(db);
        });
    });
}

fn bench_queries(c: &mut Criterion) {
    let input = generate_bibtex(1000);
    let db = Database::parse(&input).unwrap();

    c.bench_function("find_by_key", |b| {
        b.iter(|| {
            let entry = db.find_by_key(black_box("entry500"));
            black_box(entry);
        });
    });

    c.bench_function("find_by_type", |b| {
        b.iter(|| {
            let entries = db.find_by_type(black_box("article"));
            black_box(entries);
        });
    });

    c.bench_function("find_by_field", |b| {
        b.iter(|| {
            let entries = db.find_by_field(black_box("year"), black_box("2010"));
            black_box(entries);
        });
    });
}

criterion_group!(benches, bench_parsing, bench_real_files, bench_queries);
criterion_main!(benches);
