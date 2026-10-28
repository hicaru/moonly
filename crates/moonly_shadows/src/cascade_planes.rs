use bevy::prelude::*;

pub(crate) fn extract_frustum_planes(matrix: Mat4) -> [Vec4; 6] {
    let row0 = matrix.row(0);
    let row1 = matrix.row(1);
    let row2 = matrix.row(2);
    let row3 = matrix.row(3);
    [
        normalize_plane(row3 + row0),
        normalize_plane(row3 - row0),
        normalize_plane(row3 + row1),
        normalize_plane(row3 - row1),
        normalize_plane(row2),
        normalize_plane(row3 - row2),
    ]
}

fn normalize_plane(plane: Vec4) -> Vec4 {
    let length = plane.truncate().length();
    if length > 1.0e-6 {
        plane / length
    } else {
        plane
    }
}
