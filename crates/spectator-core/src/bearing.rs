use crate::types::{Cardinal, Elevation, Perspective, Position2, Position3, RelativePosition};

/// Elevation threshold in meters (above/below).
const ELEVATION_THRESHOLD: f64 = 2.0;

/// Compute the relative position of `target` from `perspective`.
///
/// `occluded` is passed in from the addon's camera visibility check.
pub fn relative_position(
    perspective: &Perspective,
    target: Position3,
    occluded: bool,
) -> RelativePosition {
    let distance = distance(perspective.position, target);
    let bdeg = bearing_deg(perspective, target);
    let bearing = to_cardinal(bdeg);
    let elev = elevation(perspective.position[1], target[1]);

    RelativePosition {
        distance,
        bearing,
        bearing_deg: bdeg,
        elevation: Some(elev),
        occluded,
    }
}

/// Compute straight-line distance between two 3D points.
pub fn distance(a: Position3, b: Position3) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Compute bearing angle in degrees from `perspective` forward to `target`.
/// 0 = ahead (aligned with forward), clockwise.
///
/// Projects both vectors onto the XZ plane (Y-up, Godot convention).
pub fn bearing_deg(perspective: &Perspective, target: Position3) -> f64 {
    // Direction vector from perspective to target on XZ plane
    let dx = target[0] - perspective.position[0];
    let dz = target[2] - perspective.position[2];

    // Forward vector on XZ plane
    let fx = perspective.forward[0];
    let fz = perspective.forward[2];

    // atan2 of the cross and dot products gives signed angle
    // cross product (2D): fx*dz - fz*dx (positive = target is to the right)
    // dot product: fx*dx + fz*dz
    let cross = fx * dz - fz * dx;
    let dot = fx * dx + fz * dz;

    // atan2(cross, dot): cross = fx*dz - fz*dx is positive when target is clockwise
    // (to the right) from forward, so atan2 directly gives clockwise-positive angle.
    let angle = cross.atan2(dot).to_degrees();

    // Normalize to 0-360 clockwise from ahead
    ((angle % 360.0) + 360.0) % 360.0
}

/// Map a bearing in degrees (0=ahead, clockwise) to an 8-direction cardinal.
/// Each direction covers a 45-degree arc centered on it.
pub fn to_cardinal(degrees: f64) -> Cardinal {
    // Normalize to 0-360
    let d = ((degrees % 360.0) + 360.0) % 360.0;
    // Each octant is 45° wide, centered on multiples of 45°
    // Boundaries are at 22.5, 67.5, 112.5, 157.5, 202.5, 247.5, 292.5, 337.5
    match d {
        d if d < 22.5 => Cardinal::Ahead,
        d if d < 67.5 => Cardinal::AheadRight,
        d if d < 112.5 => Cardinal::Right,
        d if d < 157.5 => Cardinal::BehindRight,
        d if d < 202.5 => Cardinal::Behind,
        d if d < 247.5 => Cardinal::BehindLeft,
        d if d < 292.5 => Cardinal::Left,
        d if d < 337.5 => Cardinal::AheadLeft,
        _ => Cardinal::Ahead,
    }
}

/// Compute elevation classification.
pub fn elevation(perspective_y: f64, target_y: f64) -> Elevation {
    let diff = target_y - perspective_y;
    if diff.abs() <= ELEVATION_THRESHOLD {
        Elevation::Level
    } else if diff > 0.0 {
        Elevation::Above(diff.round())
    } else {
        Elevation::Below((-diff).round())
    }
}

/// Build a Perspective from a position and yaw rotation in degrees.
/// Godot convention: 0° = facing -Z, positive yaw rotates counterclockwise
/// when viewed from above (right-hand rule around Y-up axis).
pub fn perspective_from_yaw(position: Position3, yaw_deg: f64) -> Perspective {
    // Godot: 0° yaw faces -Z
    // Positive yaw is counterclockwise around Y from above in Godot's right-hand system
    // So: forward_x = -sin(yaw), forward_z = -cos(yaw)
    let yaw_rad = yaw_deg.to_radians();
    let forward = [-yaw_rad.sin(), 0.0, -yaw_rad.cos()];

    let (facing, facing_deg) = compass_bearing(forward);

    Perspective {
        position,
        forward,
        facing,
        facing_deg,
    }
}

/// Create a Perspective from a position and explicit forward vector.
pub fn perspective_from_forward(position: Position3, forward: [f64; 3]) -> Perspective {
    let (facing, facing_deg) = compass_bearing(forward);
    Perspective {
        position,
        forward,
        facing,
        facing_deg,
    }
}

/// Global compass bearing of a forward vector.
/// Returns degrees: 0 = north (+Z in Godot), clockwise from above.
/// Godot convention: +Z is "south" in standard compass but we follow
/// Godot's coordinate system where the default forward (-Z) faces "north" by convention.
pub fn compass_bearing(forward: [f64; 3]) -> (Cardinal, f64) {
    // We define "north" as -Z (Godot's default forward direction at yaw=0)
    // compass_deg: angle from north (-Z), clockwise when viewed from above
    // north = [0, 0, -1]
    // cross product of north × forward gives rotation direction
    let nx = 0.0_f64;
    let nz = -1.0_f64;
    let fx = forward[0];
    let fz = forward[2];

    let cross = nx * fz - nz * fx; // cross product y component
    let dot = nx * fx + nz * fz;

    let angle = cross.atan2(dot).to_degrees();
    // Normalize to 0-360
    let deg = ((angle % 360.0) + 360.0) % 360.0;

    // Map to cardinal using compass directions
    // But here "north" is Ahead's equivalent in compass terms
    // We reuse to_cardinal since 0 = north = ahead (both 0°)
    let cardinal = to_cardinal(deg);

    (cardinal, deg)
}

/// Compute the relative position of a 2D target from a 2D perspective.
/// No elevation in 2D.
pub fn relative_position_2d(
    perspective_pos: Position2,
    perspective_forward: Position2,
    target: Position2,
    occluded: bool,
) -> RelativePosition {
    let distance = distance_2d(perspective_pos, target);
    let bdeg = bearing_deg_2d(perspective_pos, perspective_forward, target);
    let bearing = to_cardinal(bdeg);

    RelativePosition {
        distance,
        bearing,
        bearing_deg: bdeg,
        elevation: None,
        occluded,
    }
}

/// 2D Euclidean distance.
pub fn distance_2d(a: Position2, b: Position2) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

/// 2D bearing angle in degrees from perspective forward to target.
/// 0 = ahead, clockwise.
/// Godot 2D convention: X right, Y down. Rotation 0° = facing right (+X).
pub fn bearing_deg_2d(
    perspective_pos: Position2,
    perspective_forward: Position2,
    target: Position2,
) -> f64 {
    let dx = target[0] - perspective_pos[0];
    let dy = target[1] - perspective_pos[1];

    let fx = perspective_forward[0];
    let fy = perspective_forward[1];

    // 2D cross product: fx*dy - fy*dx (positive = target is clockwise from forward)
    let cross = fx * dy - fy * dx;
    let dot = fx * dx + fy * dy;

    let angle = cross.atan2(dot).to_degrees();
    ((angle % 360.0) + 360.0) % 360.0
}

/// Build a 2D perspective from position and rotation angle (degrees).
/// Godot 2D: 0° = facing right (+X), positive = clockwise.
pub fn perspective_from_angle_2d(position: Position2, angle_deg: f64) -> (Position2, Position2) {
    let rad = angle_deg.to_radians();
    let forward = [rad.cos(), rad.sin()];
    (position, forward)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_3d() {
        assert!((distance([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]) - 5.0).abs() < 1e-10);
        assert!((distance([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])).abs() < 1e-10);
    }

    #[test]
    fn cardinal_boundaries() {
        assert_eq!(to_cardinal(0.0), Cardinal::Ahead);
        assert_eq!(to_cardinal(22.4), Cardinal::Ahead);
        assert_eq!(to_cardinal(22.6), Cardinal::AheadRight);
        assert_eq!(to_cardinal(45.0), Cardinal::AheadRight);
        assert_eq!(to_cardinal(67.4), Cardinal::AheadRight);
        assert_eq!(to_cardinal(90.0), Cardinal::Right);
        assert_eq!(to_cardinal(180.0), Cardinal::Behind);
        assert_eq!(to_cardinal(270.0), Cardinal::Left);
        assert_eq!(to_cardinal(359.9), Cardinal::Ahead);
        assert_eq!(to_cardinal(360.0), Cardinal::Ahead);
    }

    #[test]
    fn elevation_above_below_level() {
        assert_eq!(elevation(0.0, 5.0), Elevation::Above(5.0));
        assert_eq!(elevation(0.0, 1.5), Elevation::Level);
        assert_eq!(elevation(10.0, 3.0), Elevation::Below(7.0));
        assert_eq!(elevation(0.0, 2.0), Elevation::Level);
        assert_eq!(elevation(0.0, -2.0), Elevation::Level);
        assert_eq!(elevation(0.0, 2.1), Elevation::Above(2.0));
    }

    #[test]
    fn bearing_ahead_when_aligned() {
        // Perspective at origin, facing -Z (yaw=0, Godot convention)
        let persp = perspective_from_yaw([0.0, 0.0, 0.0], 0.0);
        // Target directly ahead: -Z direction
        let target = [0.0, 0.0, -10.0];
        let bdeg = bearing_deg(&persp, target);
        assert!(bdeg < 1.0 || bdeg > 359.0, "Expected ~0°, got {bdeg}");
    }

    #[test]
    fn bearing_right_when_perpendicular() {
        // Perspective at origin, facing -Z (yaw=0)
        let persp = perspective_from_yaw([0.0, 0.0, 0.0], 0.0);
        // Target to the right: +X direction
        let target = [10.0, 0.0, 0.0];
        let bdeg = bearing_deg(&persp, target);
        assert!((bdeg - 90.0).abs() < 1.0, "Expected ~90°, got {bdeg}");
    }

    #[test]
    fn godot_coordinate_convention() {
        // Verify Y-up, -Z forward at yaw=0
        let persp = perspective_from_yaw([0.0, 0.0, 0.0], 0.0);
        // Forward should be [0, 0, -1]
        assert!((persp.forward[0]).abs() < 1e-10);
        assert!((persp.forward[2] + 1.0).abs() < 1e-10);

        // Target directly ahead (-Z) should produce ~0° bearing
        let bdeg = bearing_deg(&persp, [0.0, 0.0, -10.0]);
        assert!(bdeg < 1.0 || bdeg > 359.0, "ahead should be ~0°, got {bdeg}");

        // Target behind (+Z) should produce ~180°
        let bdeg_behind = bearing_deg(&persp, [0.0, 0.0, 10.0]);
        assert!((bdeg_behind - 180.0).abs() < 1.0, "behind should be ~180°, got {bdeg_behind}");
    }

    #[test]
    fn compass_bearing_north() {
        // Facing -Z (default Godot forward) = north = 0°
        let (cardinal, deg) = compass_bearing([0.0, 0.0, -1.0]);
        assert!((deg).abs() < 1.0 || (deg - 360.0).abs() < 1.0, "Expected 0°, got {deg}");
        assert_eq!(cardinal, Cardinal::Ahead);
    }

    #[test]
    fn perspective_from_forward_negative_z() {
        let p = perspective_from_forward([0.0, 0.0, 0.0], [0.0, 0.0, -1.0]);
        assert_eq!(p.facing, Cardinal::Ahead);
        assert!(p.facing_deg.abs() < 1.0 || (p.facing_deg - 360.0).abs() < 1.0);
    }

    #[test]
    fn perspective_from_yaw_90deg() {
        // 90° yaw in Godot = facing -X
        let persp = perspective_from_yaw([0.0, 0.0, 0.0], 90.0);
        assert!((persp.forward[0] + 1.0).abs() < 1e-6, "forward_x should be -1, got {}", persp.forward[0]);
        assert!((persp.forward[2]).abs() < 1e-6, "forward_z should be 0, got {}", persp.forward[2]);
    }

    #[test]
    fn distance_2d_basic() {
        assert!((distance_2d([0.0, 0.0], [3.0, 4.0]) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn bearing_2d_ahead() {
        // Facing right (+X), target directly ahead
        let bdeg = bearing_deg_2d([0.0, 0.0], [1.0, 0.0], [10.0, 0.0]);
        assert!(bdeg < 1.0 || bdeg > 359.0, "Expected ~0°, got {bdeg}");
    }

    #[test]
    fn bearing_2d_right() {
        // Facing right (+X), target below (+Y in Godot 2D = clockwise = right)
        let bdeg = bearing_deg_2d([0.0, 0.0], [1.0, 0.0], [0.0, 10.0]);
        assert!((bdeg - 90.0).abs() < 1.0, "Expected ~90°, got {bdeg}");
    }

    #[test]
    fn relative_position_2d_no_elevation() {
        let rel = relative_position_2d([0.0, 0.0], [1.0, 0.0], [10.0, 0.0], false);
        assert!(rel.elevation.is_none());
        assert!(rel.distance > 9.9);
    }
}
