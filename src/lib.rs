use posh::{
    gl,
    sl::{self, *},
    Block, BlockDom, Sl,
};

#[derive(Clone, Copy, Block)]
#[repr(C)]
pub struct Uniforms<D: BlockDom> {
    pub time: D::F32,
    pub size: D::Vec2,
}

pub fn vertex_shader(globals: Uniforms<Sl>, vertex: sl::Vec2) -> sl::VsOutput<sl::Vec2> {
    sl::VsOutput {
        clip_position: sl::vec4(vertex.x, vertex.y, 0.0, 1.0),
        interpolant: vertex,
    }
}

pub fn uv(clip_space_pos: Vec2) -> Vec2 {
    clip_space_pos * 0.5 + 0.5
}

pub fn fragment_shader(Uniforms::<Sl> { time, size }: Uniforms<Sl>, clip_space_pos: Vec2) -> Vec4 {
    let uv = uv(clip_space_pos);
    sl::vec4(uv.x, uv.y, 1.0, 1.0)
}
