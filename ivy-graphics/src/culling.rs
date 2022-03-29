use glam::{Mat4, Vec4Swizzles};
use hecs_schedule::SubWorld;
use ivy_base::{Position, Visible, WorldExt};

use crate::{Camera, MainCamera};

pub fn visible(pos: Position, viewproj: Mat4) -> bool {
    let clip = viewproj * pos.extend(1.0);
    let clip = clip.xyz() / clip.w;
    clip.x > -1.0 && clip.x < 1.0 && clip.y > -1.0 && clip.y < 1.0
}
