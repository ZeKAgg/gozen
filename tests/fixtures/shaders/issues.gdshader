shader_type spatial;

//bad comment spacing
uniform vec4 unused_color : source_color = vec4(1.0);
varying vec3 unused_varying;

float helper_unused() {
	return 0.5;
}

void fragment() {
	float val = .5;
	ALBEDO = vec3(val);
}
