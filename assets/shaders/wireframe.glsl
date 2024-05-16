varying vec2 v_uv;
varying vec4 v_color;

#ifdef VERTEX_SHADER
attribute vec3 a_pos;
attribute vec2 a_uv;
attribute vec4 a_color;

uniform mat4 u_view_matrix;
uniform mat4 u_projection_matrix;

void main() {
  v_uv = a_uv;
  v_color = a_color;
  vec4 camera_pos = u_view_matrix * vec4(a_pos, 1.0);
  camera_pos.z += 1e-2;
  gl_Position = u_projection_matrix * camera_pos;
}
#endif

#ifdef FRAGMENT_SHADER
uniform vec4 u_color;

void main() {
  gl_FragColor = u_color;
}
#endif
