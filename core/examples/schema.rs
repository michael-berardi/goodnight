fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&carapace::schema_with_queries::<goodnight_core::GoodNight>())
            .unwrap()
    );
}
