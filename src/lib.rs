use posh::{
    gl,
    sl::{self, *},
    Block, BlockDom, Sl, UniformInterface, UniformInterfaceDom,
};

#[derive(Block, Clone, Copy, Default)]
#[repr(C)]
pub struct App<D: BlockDom> {
    pub window_size: D::Vec2,
}

#[derive(UniformInterface, Clone)]
pub struct Uniforms<D: UniformInterfaceDom> {
    pub texture: D::ColorSampler2d<Vec4>,
    pub app: D::Block<App<Sl>>,
}

pub fn vertex_shader(globals: Uniforms<Sl>, vertex: sl::Vec2) -> sl::VsOutput<sl::Vec2> {
    sl::VsOutput {
        clip_position: sl::vec4(vertex.x, vertex.y, 0.0, 1.0),
        interpolant: vertex,
    }
}

pub fn fragcoord(clip_space_pos: Vec2, window_size: Vec2) -> Vec2 {
    uv(clip_space_pos) * window_size
}

pub fn uv(clip_space_pos: Vec2) -> Vec2 {
    clip_space_pos * 0.5 + 0.5
}

pub fn flip_v(uv: Vec2) -> Vec2 {
    Vec2::new(uv.x, 1.0 - uv.y)
}

#[derive(Debug)]
enum ErrorKind {
    InvalidSize,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

impl std::error::Error for ErrorKind {}

pub fn texture_aspect_ratio<C: ColorSample>(sampler: ColorSampler2d<C>) -> F32 {
    let size: Vec2 = sampler.size(0u32).as_vec2();
    aspect_ratio(size)
}

pub fn aspect_ratio(size: Vec2) -> F32 {
    size.x / size.y
}

pub fn preserve_aspect_ratio(viewport_aspect: F32, texture_aspect: F32, uv: Vec2) -> Vec2 {
    branch(
        viewport_aspect.ge(texture_aspect),
        Vec2::new(uv.x * (viewport_aspect / texture_aspect), uv.y),
        Vec2::new(uv.x, uv.y * (texture_aspect / viewport_aspect)),
    )
}

pub fn fragment_shader(
    Uniforms::<Sl> { texture, app }: Uniforms<Sl>,
    clip_space_pos: Vec2,
) -> Vec4 {
    let uv = uv(clip_space_pos);
    let fragcoord = fragcoord(clip_space_pos, app.window_size);
    let viewport_aspect = aspect_ratio(app.window_size);
    let texture_aspect = texture_aspect_ratio(texture);

    let uv = preserve_aspect_ratio(viewport_aspect, texture_aspect, uv);
    let uv = flip_v(uv);

    let color = texture.sample(uv);
    let step = uv.step(1.0) + (uv * -1.0).step(0.0);

    color.lerp(Vec4::new(0.0, 0.0, 0.0, 1.0), step.x + step.y)
    //Vec4::new(uv.x, uv.y, 0.0, 1.0)
}
