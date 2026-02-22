// spatial_shader.gdshader — A well-formatted spatial shader.
// Gozen enforces the Godot Shaders Style Guide automatically.

shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_back;

// Constants use SCREAMING_SNAKE_CASE
const float MAX_DISTANCE = 100.0;
const float FADE_START = 80.0;

// Uniforms use snake_case, float literals always have digits on both sides
uniform vec4 albedo_color : source_color = vec4(1.0, 1.0, 1.0, 1.0);
uniform sampler2D albedo_texture : source_color;
uniform float roughness : hint_range(0.0, 1.0) = 0.5;

// Varyings for passing data between vertex and fragment
varying vec3 world_position;

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	// Sample the texture
	vec4 tex = texture(albedo_texture, UV);
	ALBEDO = tex.rgb * albedo_color.rgb;
	ROUGHNESS = roughness;

	// Distance-based fade
	float dist = length(world_position - CAMERA_POSITION_WORLD);
	if (dist > FADE_START) {
		float fade = clamp((dist - FADE_START) / (MAX_DISTANCE - FADE_START), 0.0, 1.0);
		ALPHA = 1.0 - fade;
	}
}
