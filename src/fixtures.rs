// src/fixtures.rs
// Realistic BibTeX test fixtures based on actual academic entries
// 
// These fixtures represent real-world entry patterns from major venues:
// - NeurIPS, ICML, ICLR (ML conferences)
// - Physical Review, Neural Computation (journals)
// - Annual Reviews (review articles with abstracts)
// - PMLR (proceedings with long abstracts)
// - Misc entries (unpublished work)

/// String constants for common publishers/series
pub static COMMON_STRINGS: &str = r#"
@string{neurips = "Advances in Neural Information Processing Systems"}
@string{icml = "International Conference on Machine Learning"}
@string{iclr = "International Conference on Learning Representations"}
@string{pmlr = "Proceedings of Machine Learning Research"}
@string{curran = "Curran Associates, Inc."}
@string{aps = "American Physical Society"}
@string{mit = "MIT Press"}
@string{annrev = "Annual Reviews"}
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
        name: "neurips_2024",
        venue: "NeurIPS",
        content: r#"@inproceedings{sun2024attributing,
    author = {Sun, Yifan and Shen, Jingyan and Kwon, Yongchan},
    booktitle = neurips,
    editor = {A. Globerson and L. Mackey and D. Belgrave and A. Fan and U. Paquet and J. Tomczak and C. Zhang},
    pages = {46764--46790},
    publisher = curran,
    title = {2D-OOB: Attributing Data Contribution Through Joint Valuation Framework},
    url = {https://proceedings.neurips.cc/paper_files/paper/2024/file/531998dc1fc858b5857a90b74d96ecab-Paper-Conference.pdf},
    volume = {37},
    year = {2024}
}"#,
        approx_size: 520,
        field_count: 9,
    },
    TestEntry {
        name: "icml_2021",
        venue: "ICML",
        content: r#"@inproceedings{acar2021debiasing,
    address = {Virtual Event},
    author = {Acar, Durmus Alp Emre and Zhao, Yue and Zhu, Ruizhao and Navarro, Ramon Matas and Mattina, Matthew and Whatmough, Paul N. and Saligrama, Venkatesh},
    booktitle = {Proceedings of the 38th International Conference on Machine Learning (ICML)},
    editor = {Meila, Marina and Zhang, Tong},
    month = {July},
    note = {\url{http://proceedings.mlr.press/v139/acar21a.html}},
    pages = {21--31},
    publisher = pmlr,
    series = pmlr,
    title = {Debiasing Model Updates for Improving Personalized Federated Training},
    volume = {139},
    year = {2021}
}"#,
        approx_size: 640,
        field_count: 12,
    },
    TestEntry {
        name: "phys_rev_2024",
        venue: "Physical Review A",
        content: r#"@article{li2024anomalous,
    author = {Li, Shuai and Liu, Min and Zhang, Yue and Tian, Rui and Arzamasovs, Maksims and Liu, Bo},
    doi = {10.1103/PhysRevA.110.042208},
    issue = {4},
    journal = {Phys. Rev. A},
    month = {Oct},
    numpages = {6},
    openalex = {W4403204543},
    pages = {042208},
    publisher = aps,
    title = {Anomalous symmetry-protected blockade of the skin effect in one-dimensional non-Hermitian lattice systems},
    url = {https://link.aps.org/doi/10.1103/PhysRevA.110.042208},
    volume = {110},
    year = {2024}
}"#,
        approx_size: 590,
        field_count: 12,
    },
    TestEntry {
        name: "neural_comp_2023",
        venue: "Neural Computation",
        content: r#"@article{radulescu2023synchronization,
    author = {Rǎdulescu, Anca and Evans, Danae and Augustin, Amani-Dasia and Cooper, Anthony and Nakuci, Johan and Muldoon, Sarah},
    doi = {10.1162/neco_a_01624},
    eprint = {https://direct.mit.edu/neco/article-pdf/36/1/75/2195597/neco_a_01624.pdf},
    issn = {0899-7667},
    journal = {Neural Computation},
    month = {12},
    number = {1},
    pages = {75-106},
    title = {Synchronization and Clustering in Complex Quadratic Networks},
    url = {https://doi.org/10.1162/neco_a_01624},
    volume = {36},
    year = {2023}
}"#,
        approx_size: 580,
        field_count: 11,
    },
    TestEntry {
        name: "annual_review_2024",
        venue: "Annual Review of Statistics",
        content: r#"@article{bianchi2024relational,
    abstract = {Advances in information technology have increased the availability of time-stamped relational data, such as those produced by email exchanges or interaction through social media. Whereas the associated information flows could be aggregated into cross-sectional panels, the temporal ordering of the events frequently contains information that requires new models for the analysis of continuous-time interactions, subject to both endogenous and exogenous influences. The introduction of the relational event model (REM) has been a major development that has stimulated new questions and led to further methodological developments. In this review, we track the intellectual history of the REM, define its core properties, and discuss why and how it has been considered useful in empirical research. We describe how the demands of novel applications have stimulated methodological, computational, and inferential advancements.},
    author = {Bianchi, Federica and Filippi-Mazzola, Edoardo and Lomi, Alessandro and Wit, Ernst C.},
    doi = {https://doi.org/10.1146/annurev-statistics-040722-060248},
    issn = {2326-831X},
    journal = {Annual Review of Statistics and Its Application},
    keywords = {social network analysis},
    number = {Volume 11, 2024},
    pages = {297-319},
    publisher = annrev,
    title = {Relational Event Modeling},
    type = {Journal Article},
    url = {https://www.annualreviews.org/content/journals/10.1146/annurev-statistics-040722-060248},
    volume = {11},
    year = {2024}
}"#,
        approx_size: 1400,
        field_count: 12,
    },
    TestEntry {
        name: "pmlr_2023",
        venue: "PMLR",
        content: r#"@InProceedings{mundt2023continual,
    title = {Continual Causality: A Retrospective of the Inaugural AAAI-23 Bridge Program},
    author = {Mundt, Martin and Cooper, Keiland W. and Dhami, Devendra Singh and Ribeiro, Ad\'ele and Smith, James Seale and Bellot, Alexis and Hayes, Tyler},
    booktitle = {Proceedings of The First AAAI Bridge Program on Continual Causality},
    pages = {1--10},
    year = {2023},
    editor = {Mundt, Martin and Cooper, Keiland W. and Dhami, Devendra Singh and Ribeiro, Adéle and Smith, James Seale and Bellot, Alexis and Hayes, Tyler},
    volume = {208},
    series = pmlr,
    month = {07--08 Feb},
    publisher = pmlr,
    pdf = {https://proceedings.mlr.press/v208/mundt23a/mundt23a.pdf},
    url = {https://proceedings.mlr.press/v208/mundt23a.html},
    abstract = {Both of the fields of continual learning and causality investigate complementary aspects of human cognition and are fundamental components of artificial intelligence if it is to reason and generalize in complex environments. Despite the burgeoning interest in investigating the intersection of the two fields, it is currently unclear how causal models may describe continuous streams of data and vice versa, how continual learning may exploit learned causal structure. We proposed to bridge this gap through the inaugural AAAI-23 "Continual Causality" bridge program, where our aim was to take the initial steps towards a unified treatment of these fields by providing a space for learning, discussions, and to build a diverse community to connect researchers. The activities ranged from traditional tutorials and software labs, invited vision talks, and contributed talks based on submitted position papers, as well as a panel and breakout discussions. Whereas materials are publicly disseminated as a foundation for the community: https://www.continualcausality.org, respectively discussed ideas, challenges, and prospects beyond the inaugural bridge are summarized in this retrospective paper.}
}"#,
        approx_size: 1700,
        field_count: 13,
    },
];

/// Generate a realistic BibTeX file with n entries
pub fn generate_realistic_bibtex(n_entries: usize) -> String {
    let mut bib = String::with_capacity(n_entries * 700); // ~700 bytes per entry
    
    // Add string definitions
    bib.push_str(COMMON_STRINGS);
    bib.push('\n');
    
    // Cycle through venue types for variety
    let venues = VENUE_ENTRIES;
    
    for i in 0..n_entries {
        let template = &venues[i % venues.len()];
        
        // Modify the entry slightly to make it unique
        let entry = template.content
            .replace("2024", &format!("{}", 2020 + (i % 5)))
            .replace("2023", &format!("{}", 2019 + (i % 5)))
            .replace("2021", &format!("{}", 2018 + (i % 5)))
            .replace("Volume 11", &format!("Volume {}", 10 + (i % 5)))
            .replace("volume = {37}", &format!("volume = {{{}}}", 35 + (i % 5)))
            .replace("pages = {1--10}", &format!("pages = {{{}--{}}}", i * 10 + 1, i * 10 + 10));
        
        // Update the citation key to be unique
        let key_line = entry.lines().next().unwrap();
        let new_key = format!("entry{}", i);
        let updated_entry = entry.replacen(
            &key_line[key_line.find('{').unwrap() + 1..key_line.find(',').unwrap()],
            &new_key,
            1
        );
        
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

#[cfg(test)]
mod tests {
    
    #[test]
    fn test_realistic_generation() {
        let bib = generate_realistic_bibtex(10);
        assert!(bib.len() > 6000); // Should be ~7000 bytes
        assert!(bib.contains("@string"));
        assert!(bib.contains("entry0"));
        assert!(bib.contains("entry9"));
    }
    
    #[test]
    fn test_average_size() {
        let avg = average_bytes_per_entry();
        assert!(avg > 500); // Real entries are 500-700 bytes
        assert!(avg < 1000);
    }
}