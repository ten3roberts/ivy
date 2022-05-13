struct VertexOutput {
  [[builtin(position)]] clip_pos: vec4<f32>;
};


struct VertexInput {
  [[location(0)]] pos: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main(vertex: VertexInput) -> VertexOutput {
  var out: VertexOutput;
  out.clip_pos = vec4<f32>(vertex.pos, 1.0);
  return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  return vec4<f32>(0.2, 0.0, 0.5, 1.0);
}
