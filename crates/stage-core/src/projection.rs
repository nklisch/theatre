//! Godot-convention camera projection used by recorded visual artifacts.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraProjection {
    Perspective,
    Orthogonal,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeepAspect {
    KeepWidth,
    KeepHeight,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    pub position: [f64; 3],
    /// Godot quaternion order: x, y, z, w.
    pub quaternion: [f64; 4],
    pub projection: CameraProjection,
    pub fov_deg: f64,
    pub ortho_size: f64,
    pub keep_aspect: KeepAspect,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScreenProjection {
    OnScreen { px: f64, py: f64 },
    OffScreen { px: f64, py: f64 },
    BehindCamera,
}

pub fn project_world_to_screen(
    pose: CameraPose,
    world: [f64; 3],
    width: f64,
    height: f64,
) -> ScreenProjection {
    let [x, y, z, w] = pose.quaternion;
    let qn = (x * x + y * y + z * z + w * w).sqrt();
    if !qn.is_finite()
        || qn == 0.0
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return ScreenProjection::BehindCamera;
    }
    let (x, y, z, w) = (x / qn, y / qn, z / qn, w / qn);
    let d = [
        world[0] - pose.position[0],
        world[1] - pose.position[1],
        world[2] - pose.position[2],
    ];
    // R^T * d, expanded from the quaternion rotation matrix.
    let right = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - z * w),
        2.0 * (x * z + y * w),
    ];
    let up = [
        2.0 * (x * y + z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - x * w),
    ];
    let back = [
        2.0 * (x * z - y * w),
        2.0 * (y * z + x * w),
        1.0 - 2.0 * (x * x + y * y),
    ];
    let cam = [dot(right, d), dot(up, d), dot(back, d)];
    let (nx, ny) = match pose.projection {
        CameraProjection::Orthogonal => {
            let (half_w, half_h) = ortho_extents(pose.ortho_size, width, height, pose.keep_aspect);
            (cam[0] / half_w, cam[1] / half_h)
        }
        CameraProjection::Perspective => {
            if cam[2] >= 0.0 {
                return ScreenProjection::BehindCamera;
            }
            let tan = (pose.fov_deg.to_radians() / 2.0).tan();
            if !tan.is_finite() || tan <= 0.0 {
                return ScreenProjection::BehindCamera;
            }
            let (half_w, half_h) = match pose.keep_aspect {
                KeepAspect::KeepHeight => ((-cam[2]) * tan * (width / height), (-cam[2]) * tan),
                KeepAspect::KeepWidth => ((-cam[2]) * tan, (-cam[2]) * tan * (height / width)),
            };
            (cam[0] / half_w, cam[1] / half_h)
        }
    };
    let px = (nx + 1.0) * width * 0.5;
    let py = (1.0 - ny) * height * 0.5;
    if px >= 0.0 && px <= width && py >= 0.0 && py <= height {
        ScreenProjection::OnScreen { px, py }
    } else {
        ScreenProjection::OffScreen { px, py }
    }
}
fn ortho_extents(size: f64, width: f64, height: f64, aspect: KeepAspect) -> (f64, f64) {
    match aspect {
        KeepAspect::KeepHeight => (size * width / height * 0.5, size * 0.5),
        KeepAspect::KeepWidth => (size * 0.5, size * height / width * 0.5),
    }
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p() -> CameraPose {
        CameraPose {
            position: [0.0; 3],
            quaternion: [0.0, 0.0, 0.0, 1.0],
            projection: CameraProjection::Perspective,
            fov_deg: 90.0,
            ortho_size: 10.0,
            keep_aspect: KeepAspect::KeepHeight,
        }
    }
    #[test]
    fn center() {
        assert_eq!(
            project_world_to_screen(p(), [0., 0., -1.], 100., 100.),
            ScreenProjection::OnScreen { px: 50., py: 50. }
        );
    }
    #[test]
    fn behind() {
        assert_eq!(
            project_world_to_screen(p(), [0., 0., 1.], 100., 100.),
            ScreenProjection::BehindCamera
        );
    }
    #[test]
    fn offscreen() {
        assert!(matches!(
            project_world_to_screen(p(), [2., 0., -1.], 100., 100.),
            ScreenProjection::OffScreen { .. }
        ));
    }
    #[test]
    fn orthogonal() {
        let mut c = p();
        c.projection = CameraProjection::Orthogonal;
        c.ortho_size = 2.;
        assert_eq!(
            project_world_to_screen(c, [0., 0., -1.], 100., 100.),
            ScreenProjection::OnScreen { px: 50., py: 50. }
        );
    }
}
