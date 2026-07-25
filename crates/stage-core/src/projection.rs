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
    // cam = B^T * d, where B's COLUMNS are the camera's world-space axes
    // (right, up, back). The expansions below are those columns of R(q);
    // using R's rows here computes R*d instead of B^T*d and is wrong for
    // every non-identity rotation (identity is symmetric, so it slips tests).
    let right = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
    ];
    let up = [
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
    ];
    let back = [
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
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

    /// Quaternion for a yaw of `deg` about +Y (Godot convention).
    fn yaw(deg: f64) -> [f64; 4] {
        let h = deg.to_radians() / 2.0;
        [0.0, h.sin(), 0.0, h.cos()]
    }

    fn assert_centered(r: ScreenProjection) {
        match r {
            ScreenProjection::OnScreen { px, py } => {
                assert!((px - 50.0).abs() < 1e-6, "px={px}");
                assert!((py - 50.0).abs() < 1e-6, "py={py}");
            }
            other => panic!("expected centered OnScreen, got {other:?}"),
        }
    }

    #[test]
    fn yaw90_camera_looks_down_negative_x() {
        // Godot Camera3D at origin with rotation_degrees = (0, 90, 0) faces -X.
        let mut c = p();
        c.quaternion = yaw(90.0);
        // Node directly ahead -> centered.
        assert_centered(project_world_to_screen(c, [-1., 0., 0.], 100., 100.));
        // Node behind the camera (+X) -> BehindCamera.
        assert_eq!(
            project_world_to_screen(c, [1., 0., 0.], 100., 100.),
            ScreenProjection::BehindCamera
        );
        // Camera right points toward -Z (R(90°Y)·(1,0,0) = (0,0,-1)). A node
        // forward-left of that axis, (-2, 0, -1), is in the frustum on the
        // camera's right: cam.x = 1, depth = 2, nx = 0.5 -> px = 75.
        let r = project_world_to_screen(c, [-2., 0., -1.], 100., 100.);
        match r {
            ScreenProjection::OnScreen { px, .. } => {
                assert!((px - 75.0).abs() < 1e-6, "expected px=75, got {px}")
            }
            other => panic!("expected OnScreen px=75, got {other:?}"),
        }
    }

    #[test]
    fn yaw180_camera_looks_down_positive_z() {
        let mut c = p();
        c.quaternion = yaw(180.0);
        assert_centered(project_world_to_screen(c, [0., 0., 1.], 100., 100.));
        assert_eq!(
            project_world_to_screen(c, [0., 0., -1.], 100., 100.),
            ScreenProjection::BehindCamera
        );
    }

    #[test]
    fn yaw45_projects_diagonal_correctly() {
        // Camera yawed 45° faces (-sin45, 0, -cos45). A node exactly on that
        // axis is centered; a node on the old -Z axis sits at the right edge:
        // camera-space x = sin45, depth = cos45, fov 90° aspect 1 → x_ndc = 1.
        let mut c = p();
        c.quaternion = yaw(45.0);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        assert_centered(project_world_to_screen(c, [-s, 0., -s], 100., 100.));
        let r = project_world_to_screen(c, [0., 0., -1.], 100., 100.);
        match r {
            ScreenProjection::OnScreen { px, .. } | ScreenProjection::OffScreen { px, .. } => {
                assert!(
                    (px - 100.0).abs() < 1e-6,
                    "expected right edge, got px={px}"
                );
            }
            other => panic!("expected projected point, got {other:?}"),
        }
    }
}
