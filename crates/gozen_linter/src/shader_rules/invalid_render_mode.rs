use gozen_diagnostics::{Diagnostic, Severity, Span};
use gozen_parser::{node_text, Tree};

use crate::rule::RuleMetadata;
use crate::shader_rule::ShaderRule;

// Render modes valid per shader type (non-exhaustive set from Godot docs)
const SPATIAL_MODES: &[&str] = &[
    "blend_mix",
    "blend_add",
    "blend_sub",
    "blend_mul",
    "depth_draw_opaque",
    "depth_draw_always",
    "depth_draw_never",
    "depth_prepass_alpha",
    "depth_test_disabled",
    "cull_back",
    "cull_front",
    "cull_disabled",
    "unshaded",
    "wireframe",
    "diffuse_lambert",
    "diffuse_lambert_wrap",
    "diffuse_burley",
    "diffuse_toon",
    "specular_schlick_ggx",
    "specular_toon",
    "specular_disabled",
    "skip_vertex_transform",
    "world_vertex_coords",
    "ensure_correct_normals",
    "shadows_disabled",
    "ambient_light_disabled",
    "shadow_to_opacity",
    "vertex_lighting",
    "particle_trails",
    "alpha_to_coverage",
    "alpha_to_coverage_and_one",
    "fog_disabled",
];

const CANVAS_ITEM_MODES: &[&str] = &[
    "blend_mix",
    "blend_add",
    "blend_sub",
    "blend_mul",
    "blend_premul_alpha",
    "blend_disabled",
    "unshaded",
    "light_only",
    "skip_vertex_transform",
    "particle_trails",
];

const PARTICLES_MODES: &[&str] = &[
    "keep_data",
    "disable_velocity",
    "disable_force",
    "collision_use_scale",
];

const SKY_MODES: &[&str] = &["use_half_res_pass", "use_quarter_res_pass"];

const FOG_MODES: &[&str] = &[];

pub struct InvalidRenderMode;

impl ShaderRule for InvalidRenderMode {
    fn metadata(&self) -> &RuleMetadata {
        &RuleMetadata {
            id: "shader/invalidRenderMode",
            name: "invalidRenderMode",
            group: "shader",
            default_severity: Severity::Error,
            has_fix: false,
            description: "Render mode not valid for the declared shader type.",
            explanation: "Each shader type supports a specific set of render modes. Using an invalid render mode will cause a shader compilation error in Godot.",
        }
    }

    fn check(&self, tree: &Tree, source: &str) -> Vec<Diagnostic> {
        let root = tree.root_node();

        // First find the shader type
        let mut shader_type: Option<String> = None;
        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "shader_type_declaration" {
                    let text = node_text(child, source);
                    let trimmed = text.trim().trim_end_matches(';').trim();
                    if let Some(t) = trimmed.strip_prefix("shader_type") {
                        shader_type = Some(t.trim().to_string());
                    }
                }
            }
        }

        let shader_type = match shader_type {
            Some(t) => t,
            None => return Vec::new(), // No shader type — other rule will catch this
        };

        let valid_modes: &[&str] = match shader_type.as_str() {
            "spatial" => SPATIAL_MODES,
            "canvas_item" => CANVAS_ITEM_MODES,
            "particles" => PARTICLES_MODES,
            "sky" => SKY_MODES,
            "fog" => FOG_MODES,
            _ => return Vec::new(),
        };

        let mut diags = Vec::new();
        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                if child.kind() == "render_mode_declaration" {
                    let text = node_text(child, source);
                    let trimmed = text.trim().trim_end_matches(';').trim();
                    if let Some(modes_str) = trimmed.strip_prefix("render_mode") {
                        for mode in modes_str.split(',') {
                            let mode = mode.trim();
                            if !mode.is_empty() && !valid_modes.contains(&mode) {
                                diags.push(Diagnostic {
                                    severity: Severity::Error,
                                    message: format!(
                                        "Render mode `{}` is not valid for shader type `{}`.",
                                        mode, shader_type
                                    ),
                                    file_path: None,
                                    rule_id: None,
                                    span: Span {
                                        start_byte: child.start_byte(),
                                        end_byte: child.end_byte(),
                                        start_row: child.start_position().row,
                                        start_col: child.start_position().column,
                                        end_row: child.end_position().row,
                                        end_col: child.end_position().column,
                                    },
                                    notes: Vec::new(),
                                    fix: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        diags
    }
}
