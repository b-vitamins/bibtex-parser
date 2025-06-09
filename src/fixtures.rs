// src/fixtures.rs
// Realistic BibTeX test fixtures based on actual academic entries
// Simplified for compatibility with multiple parsers

/// String constants for common publishers/series
pub static COMMON_STRINGS: &str = r#"
@string{neurips = "Advances in Neural Information Processing Systems"}
@string{icml = "International Conference on Machine Learning"}
@string{pmlr = "Proceedings of Machine Learning Research"}
@string{ieee = "IEEE Transactions"}
@string{acm = "ACM Computing Surveys"}
@string{springer = "Springer-Verlag"}
"#;

/// Representative entries from different venues
pub struct TestEntry {
    pub name: &'static str,
    pub venue: &'static str,
    pub content: &'static str,
    pub approx_size: usize,
    pub field_count: usize,
}

pub const VENUE_ENTRIES: &[TestEntry] = &[
    TestEntry {
        name: "article_ml",
        venue: "ML Journal",
        content: r#"@article{smith2024learning,
    author = {Smith, John and Doe, Jane and Johnson, Robert},
    title = {Deep Learning for Natural Language Processing: A Comprehensive Survey},
    journal = {Journal of Machine Learning Research},
    volume = {25},
    number = {3},
    pages = {123--187},
    year = {2024},
    publisher = {MIT Press},
    abstract = {This paper presents a comprehensive survey of deep learning techniques for natural language processing. We review the evolution of neural architectures from early feedforward networks to modern transformer models. Our analysis covers key applications including machine translation, sentiment analysis, and question answering systems. We also discuss current challenges and future research directions in the field.}
}"#,
        approx_size: 650,
        field_count: 10,
    },
    TestEntry {
        name: "inproceedings_conf",
        venue: "Conference",
        content: r#"@inproceedings{williams2023vision,
    author = {Williams, Alice and Brown, Bob and Davis, Carol},
    title = {Efficient Vision Transformers for Real-Time Object Detection},
    booktitle = neurips,
    pages = {5678--5689},
    year = {2023},
    publisher = pmlr,
    address = {Vancouver, Canada},
    abstract = {We present an efficient variant of vision transformers optimized for real-time object detection on edge devices. By introducing sparse attention patterns and knowledge distillation, we achieve 3x speedup with minimal accuracy loss. Experiments on COCO and ImageNet demonstrate state-of-the-art performance under computational constraints.}
}"#,
        approx_size: 550,
        field_count: 8,
    },
    TestEntry {
        name: "book_classic",
        venue: "Book",
        content: r#"@book{knuth2023art,
    author = {Knuth, Donald E.},
    title = {The Art of Computer Programming, Volume 4B: Combinatorial Algorithms},
    publisher = {Addison-Wesley},
    year = {2023},
    isbn = {978-0-13-467179-6},
    edition = {1st},
    pages = {714},
    address = {Boston, MA},
    abstract = {The latest installment in Knuth's classic series covers combinatorial algorithms with the same depth and rigor as previous volumes. Topics include satisfiability, backtracking, and dancing links.}
}"#,
        approx_size: 450,
        field_count: 9,
    },
    TestEntry {
        name: "article_physics",
        venue: "Physics",
        content: r#"@article{chen2024quantum,
    author = {Chen, Wei and Zhang, Li and Wang, Ming},
    title = {Quantum Entanglement in Many-Body Systems: Theory and Applications},
    journal = {Physical Review Letters},
    volume = {132},
    number = {8},
    pages = {082501},
    year = {2024},
    doi = {10.1103/PhysRevLett.132.082501},
    publisher = {American Physical Society},
    abstract = {We investigate quantum entanglement properties in many-body systems using tensor network methods. Our theoretical framework reveals universal scaling behavior near quantum critical points. These findings have implications for quantum computing and condensed matter physics.}
}"#,
        approx_size: 580,
        field_count: 10,
    },
    TestEntry {
        name: "techreport_ai",
        venue: "Tech Report",
        content: r#"@techreport{garcia2024llm,
    author = {Garcia, Maria and Rodriguez, Carlos and Martinez, Ana},
    title = {Scaling Laws for Large Language Models: An Empirical Study},
    institution = {OpenAI Research},
    year = {2024},
    number = {TR-2024-03},
    type = {Technical Report},
    month = {March},
    pages = {45},
    abstract = {We present an empirical study of scaling laws for large language models ranging from 1B to 100B parameters. Our experiments reveal power-law relationships between model size, dataset size, and performance across various benchmarks. We provide practical guidelines for optimal resource allocation in LLM training.}
}"#,
        approx_size: 590,
        field_count: 9,
    },
    TestEntry {
        name: "misc_preprint",
        venue: "Preprint",
        content: r#"@misc{taylor2024neural,
    author = {Taylor, Sarah and Anderson, Mark and Lee, Kevin},
    title = {Neural Architecture Search for Edge Computing: A Systematic Approach},
    year = {2024},
    eprint = {2403.12345},
    archivePrefix = {arXiv},
    primaryClass = {cs.LG},
    note = {Submitted to ICML 2024},
    abstract = {We propose a systematic approach to neural architecture search specifically designed for edge computing constraints. Our method jointly optimizes for accuracy, latency, and energy consumption. Experiments on mobile and IoT devices show significant improvements over hand-designed architectures.}
}"#,
        approx_size: 520,
        field_count: 8,
    },
];

/// Generate a realistic BibTeX file with n entries
pub fn generate_realistic_bibtex(n_entries: usize) -> String {
    let mut bib = String::with_capacity(n_entries * 600); // ~600 bytes per entry
    
    // Add string definitions
    bib.push_str(COMMON_STRINGS);
    bib.push('\n');
    
    // Cycle through venue types for variety
    let venues = VENUE_ENTRIES;
    
    for i in 0..n_entries {
        let template = &venues[i % venues.len()];
        
        // Create a unique entry by modifying the template
        let entry = template.content
            .replace("2024", &format!("{}", 2020 + (i % 5)))
            .replace("2023", &format!("{}", 2019 + (i % 5)))
            .replace("volume = {25}", &format!("volume = {{{}}}", 20 + (i % 10)))
            .replace("volume = {132}", &format!("volume = {{{}}}", 130 + (i % 5)))
            .replace("pages = {123--187}", &format!("pages = {{{}--{}}}", 100 + i * 10, 187 + i * 10))
            .replace("pages = {5678--5689}", &format!("pages = {{{}--{}}}", 5000 + i * 10, 5011 + i * 10));
        
        // Update the citation key to be unique
        let key_line = entry.lines().next().unwrap();
        let old_key = &key_line[key_line.find('{').unwrap() + 1..key_line.find(',').unwrap()];
        let new_key = format!("entry{}", i);
        let updated_entry = entry.replace(old_key, &new_key);
        
        bib.push_str(&updated_entry);
        bib.push_str("\n\n");
    }
    
    bib
}

/// Get average bytes per entry for realistic data
pub fn average_bytes_per_entry() -> usize {
    let total_size: usize = VENUE_ENTRIES.iter().map(|e| e.approx_size).sum();
    total_size / VENUE_ENTRIES.len()
}

/// Create a small test file for unit tests
pub fn small_test_file() -> &'static str {
    r#"@string{ieee = "IEEE Transactions"}

@article{test2024,
    author = {Smith, John and Doe, Jane and Johnson, Robert},
    title = {A Comprehensive Study of Machine Learning Applications in Network Security},
    journal = ieee,
    volume = {42},
    number = {3},
    pages = {123--145},
    year = {2024},
    doi = {10.1109/TNET.2024.1234567}
}

@inproceedings{conf2023,
    author = {Williams, Alice and Brown, Bob and Davis, Carol and Evans, David},
    title = {Deep Learning for Real-Time Anomaly Detection in Large-Scale Distributed Systems},
    booktitle = {Proceedings of the 40th International Conference on Machine Learning},
    pages = {5678--5689},
    year = {2023},
    publisher = {PMLR},
    url = {https://proceedings.mlr.press/v180/williams23a.html}
}
"#
}
