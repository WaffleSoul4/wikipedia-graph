use wikimedia_language_codegen::{languages_as_enum_code, languages_from_sitematrix, site_matrix, LanguageData};

fn main() {
    let site_matrix = site_matrix();

    let count = site_matrix
        .get("count")
        .expect("Failed to get page count")
        .as_u64()
        .unwrap();

    println!("Article Count: {count}");

    let languages = languages_from_sitematrix(&site_matrix);

    let code = languages_as_enum_code(languages);

    println!("{}", code.to_string());
}