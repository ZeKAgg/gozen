// Run format fixtures: each dir has input.gd and expected.gd
use std::fs;
use std::path::Path;

use gozen_config::FormatterConfig;
use gozen_formatter::format;
use gozen_parser::GDScriptParser;

fn fixtures_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("format")
}

#[test]
fn test_format_fixtures() {
    let fixtures = fixtures_dir();
    if !fixtures.exists() {
        return;
    }
    let config = FormatterConfig::default();
    let mut parser = GDScriptParser::new();
    for entry in fs::read_dir(&fixtures).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            let input_path = path.join("input.gd");
            let expected_path = path.join("expected.gd");
            if input_path.exists() && expected_path.exists() {
                let input = fs::read_to_string(&input_path).unwrap();
                let expected = fs::read_to_string(&expected_path).unwrap();
                let expected_norm = expected.replace("\r\n", "\n").replace("\r", "\n");
                let tree = parser.parse(&input).expect("parse input");
                let result = format(&input, &tree, &config);
                let result_norm = result.replace("\r\n", "\n").replace("\r", "\n");
                assert_eq!(
                    result_norm, expected_norm,
                    "Fixture {:?}: formatter output differs from expected.\n--- expected ---\n{}\n--- got ---\n{}",
                    path.file_name().unwrap(),
                    expected_norm,
                    result_norm
                );
                let tree2 = parser.parse(&result).unwrap();
                let result2 = format(&result, &tree2, &config);
                assert_eq!(
                    result,
                    result2,
                    "Fixture {:?} must be idempotent: format(format(x)) == format(x)",
                    path.file_name().unwrap()
                );
            }
        }
    }
}
