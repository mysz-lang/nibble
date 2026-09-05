use crate::out::ResultType;
use std::path::Path;

pub fn get(input_file: &str, result: ResultType) -> String {
    let path = Path::new(input_file);

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(input_file);

    let extension = match result {
        ResultType::Binary => "out",
        ResultType::Object => "o",
        ResultType::Shared => "so",
    };

    format!("{}.{}", stem, extension)
}
