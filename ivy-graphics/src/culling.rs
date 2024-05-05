use glam::{Mat4, Vec3, Vec4Swizzles};

pub fn visible(pos: Vec3, viewproj: Mat4) -> bool {
    // TODO: proper frustum culling
    let clip = viewproj * pos.extend(1.0);
    let clip = clip.xyz() / clip.w;
    clip.x > -1.0 && clip.x < 1.0 && clip.y > -1.0 && clip.y < 1.0
}
