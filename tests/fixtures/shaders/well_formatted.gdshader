shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_back;

uniform vec4 albedo_color : source_color = vec4(1.0, 1.0, 1.0, 1.0);
uniform sampler2D albedo_texture : source_color;

varying vec3 world_position;

const float MAX_DISTANCE = 100.0;

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec4 tex = texture(albedo_texture, UV);
	ALBEDO = tex.rgb * albedo_color.rgb;

	float dist = length(world_position - CAMERA_POSITION_WORLD);
	if (dist > MAX_DISTANCE) {
		ALPHA = 0.0;
	}
}
