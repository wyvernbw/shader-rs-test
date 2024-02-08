use posh::{
    gl,
    sl::{self, *},
    Block, BlockDom, Sl,
};

#[derive(Clone, Copy, Block)]
#[repr(C)]
pub struct Uniforms<D: BlockDom> {
    pub fragment: D::Vec4,
}

pub fn vertex_shader(globals: Uniforms<Sl>, vertex: sl::Vec2) -> sl::VsOutput<sl::Vec2> {
    sl::VsOutput {
        clip_position: sl::Vec4::new(vertex.x, vertex.y, 0.0, 1.0),
        interpolant: vertex,
    }
}

pub fn fragment_shader(Uniforms::<Sl> { fragment }: Uniforms<Sl>, uv: Vec2) -> Vec4 {
    fragment
}
