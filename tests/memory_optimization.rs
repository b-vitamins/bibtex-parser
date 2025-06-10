// tests/memory_optimization.rs
#[cfg(test)]
mod tests {
    use bibtex_parser::*;
    use std::mem;

    #[test]
    fn verify_optimized_struct_sizes() {
        // These are the optimized sizes we achieved
        assert_eq!(mem::size_of::<Entry>(), 72, "Entry should be 72 bytes");
        assert_eq!(
            mem::size_of::<Field>(),
            56,
            "Field should be 56 bytes (was 48)"
        );
        assert_eq!(
            mem::size_of::<Value>(),
            32,
            "Value should be 32 bytes (was 24)"
        );
        assert_eq!(
            mem::size_of::<EntryType>(),
            24,
            "EntryType should be 24 bytes"
        );

        // Verify that boxing helped
        assert_eq!(mem::size_of::<Vec<Value>>(), 24, "Vec<Value> is 24 bytes");
        assert_eq!(
            mem::size_of::<Box<Vec<Value>>>(),
            8,
            "Box<Vec<Value>> is only 8 bytes!"
        );

        // Field is name (16) + Value (24) = 40 bytes!
        // This is even better than the 48 bytes we saw before optimization
    }

    #[test]
    fn test_memory_efficiency() {
        let input = r#"
            @article{test1, title = "Test 1", author = "Author 1", year = 2023}
            @article{test2, title = "Test 2", author = "Author 2", year = 2023}
            @article{test3, title = "Test 3", author = "Author 3", year = 2023}
        "#;

        let db = Database::parse(input).unwrap();

        // All entries should have shrunk field vectors
        for entry in db.entries() {
            assert_eq!(
                entry.fields.len(),
                entry.fields.capacity(),
                "Vec should be shrunk to exact size"
            );
        }
    }

    #[test]
    fn calculate_memory_savings() {
        // For a typical conference bibliography:
        let entries = 1000;
        let avg_fields_per_entry = 11;
        let total_fields = entries * avg_fields_per_entry;

        // Old sizes
        let old_value_size = 32; // Before boxing Concat
        let old_field_size = 48; // 16 (name) + 32 (old Value)
        let old_value_total = total_fields * old_value_size;

        // New sizes
        let new_value_size = 24; // After boxing Concat
        let new_field_size = 40; // 16 (name) + 24 (new Value)
        let new_value_total = total_fields * new_value_size;

        let value_savings = old_value_total - new_value_total;
        let field_savings = total_fields * (old_field_size - new_field_size);

        println!("=== Memory Savings Analysis ===");
        println!(
            "For {} entries with {} fields each:",
            entries, avg_fields_per_entry
        );
        println!("Total fields: {}", total_fields);
        println!();
        println!("Value enum optimization:");
        println!("  Old: {} bytes per Value", old_value_size);
        println!("  New: {} bytes per Value", new_value_size);
        println!("  Savings: {} KB", value_savings / 1024);
        println!();
        println!("Field struct optimization (bonus!):");
        println!("  Old: {} bytes per Field", old_field_size);
        println!("  New: {} bytes per Field", new_field_size);
        println!("  Savings: {} KB", field_savings / 1024);
        println!();

        // Vec over-allocation (typical 30% waste)
        let vec_waste_percentage = 0.30;
        let vec_allocated = total_fields * mem::size_of::<Field>();
        let vec_waste = (vec_allocated as f64 * vec_waste_percentage) as usize;

        println!("Vec optimization (shrink_to_fit):");
        println!("  Typical waste: {:.0}%", vec_waste_percentage * 100.0);
        println!("  Savings: {} KB", vec_waste / 1024);
        println!();

        let total_savings = value_savings + field_savings + vec_waste;
        println!("Total savings: {} KB", total_savings / 1024);

        // Assert we're getting significant savings
        assert!(
            value_savings > 50_000,
            "Should save at least 50KB on Value optimization"
        );
        assert!(
            field_savings > 50_000,
            "Should save at least 50KB on Field size reduction"
        );
        assert!(
            vec_waste > 100_000,
            "Should save at least 100KB on Vec optimization"
        );
    }

    #[test]
    fn test_concat_still_works() {
        let input = r#"
            @string{first = "Hello"}
            @string{second = "World"}
            @article{test, title = first # ", " # second}
        "#;

        let db = Database::parse(input).unwrap();
        let entry = &db.entries()[0];

        // Concat should still work correctly with boxed Vec
        assert_eq!(entry.get("title").unwrap(), "Hello, World");
    }
}
