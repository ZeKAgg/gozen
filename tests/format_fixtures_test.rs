//! Format fixture tests: tests/fixtures/format/*/input.gd -> expected.gd
//! Also supports .gdshader fixtures: input.gdshader -> expected.gdshader

use std::fs;
use std::path::Path;

use gozen_config::FormatterConfig;
use gozen_formatter::{format, format_shader};
use gozen_parser::{GDScriptParser, GDShaderParser};

fn fixtures_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/format")
}

#[test]
fn test_all_format_fixtures() {
    let fixtures = fixtures_dir();
    if !fixtures.exists() {
        return;
    }
    let mut parser = GDScriptParser::new();
    let mut shader_parser = GDShaderParser::new();
    for entry in fs::read_dir(&fixtures).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            run_format_fixture(&path, &mut parser, &mut shader_parser);
        }
    }
}

fn run_format_fixture(
    fixture_dir: &Path,
    parser: &mut GDScriptParser,
    shader_parser: &mut GDShaderParser,
) {
    let config: FormatterConfig = if fixture_dir.join("config.json").exists() {
        let json = fs::read_to_string(fixture_dir.join("config.json")).unwrap();
        serde_json::from_str(&json).unwrap()
    } else {
        FormatterConfig::default()
    };

    // Try GDScript fixtures first
    let gd_input = fixture_dir.join("input.gd");
    let gd_expected = fixture_dir.join("expected.gd");
    if gd_input.exists() && gd_expected.exists() {
        let input = fs::read_to_string(&gd_input).unwrap();
        let expected = fs::read_to_string(&gd_expected).unwrap();
        let expected_norm = expected.replace("\r\n", "\n").replace('\r', "\n");

        let tree = parser.parse(&input).expect("parse GDScript input");
        let result = format(&input, &tree, &config);
        let result_norm = result.replace("\r\n", "\n").replace('\r', "\n");
        assert_eq!(
            result_norm, expected_norm,
            "Fixture {:?}: formatter output differs from expected.\n--- expected ---\n{}\n--- got ---\n{}",
            fixture_dir.file_name().unwrap(),
            expected_norm,
            result_norm
        );
        let tree2 = parser.parse(&result).unwrap();
        let result2 = format(&result, &tree2, &config);
        assert_eq!(
            result,
            result2,
            "Fixture {:?} must be idempotent: format(format(x)) == format(x)",
            fixture_dir.file_name().unwrap()
        );
    }

    // Try GDShader fixtures
    let shader_input = fixture_dir.join("input.gdshader");
    let shader_expected = fixture_dir.join("expected.gdshader");
    if shader_input.exists() && shader_expected.exists() {
        let input = fs::read_to_string(&shader_input).unwrap();
        let expected = fs::read_to_string(&shader_expected).unwrap();
        let expected_norm = expected.replace("\r\n", "\n").replace('\r', "\n");

        let tree = shader_parser.parse(&input).expect("parse GDShader input");
        let result = format_shader(&input, &tree, &config);
        let result_norm = result.replace("\r\n", "\n").replace('\r', "\n");
        assert_eq!(
            result_norm, expected_norm,
            "Shader fixture {:?}: formatter output differs from expected.\n--- expected ---\n{}\n--- got ---\n{}",
            fixture_dir.file_name().unwrap(),
            expected_norm,
            result_norm
        );
        let tree2 = shader_parser.parse(&result).unwrap();
        let result2 = format_shader(&result, &tree2, &config);
        assert_eq!(
            result,
            result2,
            "Shader fixture {:?} must be idempotent: format(format(x)) == format(x)",
            fixture_dir.file_name().unwrap()
        );
    }
}
