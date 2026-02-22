shader_type  spatial;
render_mode   blend_mix,   cull_back;

uniform   vec4   albedo_color  :  source_color  =  vec4(1.0);

void  fragment()  {
	ALBEDO  =  albedo_color.rgb;
}
