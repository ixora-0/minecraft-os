use core::mem;

use alloc::vec::Vec;
use glam::Vec2;

/// Calculates the overlap area of a polygon with a rectangular bounding box using Sutherland–Hodgman algorithm.
/// Needs two scratch buffers for intermediate storage.
pub fn overlap_area_polygon_with_rect(
    polygon: &[Vec2],
    top_left: Vec2,
    bottom_right: Vec2,
    buffer_a: &mut Vec<Vec2>,
    buffer_b: &mut Vec<Vec2>,
) -> f32 {
    buffer_a.clear();
    buffer_a.extend_from_slice(polygon);

    // polygons has > 3 vertices
    if buffer_a.len() < 3 {
        return 0.0;
    }

    let mut input = buffer_a;
    let mut output = buffer_b;

    macro_rules! clip_edge {
            (x, $cmp:tt, $bound:expr) => {
                clip_polygon_generic(input.as_slice(), output, |p| p.x $cmp $bound, |a, b| intersect_vertical(a, b, $bound));
                if output.len() < 3 {
                    return 0.0;
                }
            };
            (y, $cmp:tt, $bound:expr) => {
                clip_polygon_generic(input.as_slice(), output, |p| p.y $cmp $bound, |a, b| intersect_horizontal(a, b, $bound));
                if output.len() < 3 {
                    return 0.0;
                }
            };
        }

    // clip polygon with left edge of rect
    clip_edge!(x, >=, top_left.x);
    mem::swap(&mut input, &mut output);

    // clip polygon with left edge of rect
    clip_edge!(x, <=, bottom_right.x);
    mem::swap(&mut input, &mut output);

    // clip polygon with top edge of rect
    clip_edge!(y, >=, top_left.y);
    mem::swap(&mut input, &mut output);

    // clip polygon with bottom edge of rect
    clip_edge!(y, <=, bottom_right.y);

    polygon_area(output.as_slice())
}

/// Helper for Sutherland–Hodgman algorithm.
/// Clips any convex polygon against a single half-plane.
/// Stores the resulting clipped polygon in output.
fn clip_polygon_generic<FInside, FIntersect>(
    input: &[Vec2],
    output: &mut Vec<Vec2>,
    inside: FInside,
    intersection: FIntersect,
) where
    FInside: Fn(Vec2) -> bool,
    FIntersect: Fn(Vec2, Vec2) -> Vec2,
{
    output.clear();
    if input.is_empty() {
        return;
    }

    // start with the last vertex as previous so the polygon loops
    let mut prev = *input.last().unwrap();
    let mut prev_inside = inside(prev);

    for &curr in input {
        let curr_inside = inside(curr);

        if curr_inside {
            if !prev_inside {
                output.push(intersection(prev, curr));
            }
            output.push(curr);
        } else if prev_inside {
            output.push(intersection(prev, curr));
        }

        prev = curr;
        prev_inside = curr_inside;
    }
}

/// Calculates the area of a polygon given its vertices using the shoelace formula.
fn polygon_area(vertices: &[Vec2]) -> f32 {
    if vertices.len() < 3 {
        return 0.0;
    }

    let mut area = 0.0f32;
    let mut prev = *vertices.last().unwrap();
    for &curr in vertices {
        area += (prev.x * curr.y) - (curr.x * prev.y);
        prev = curr;
    }
    0.5 * area.abs()
}

/// Returns interection point between ab and the vertical line at x.
fn intersect_vertical(a: Vec2, b: Vec2, x: f32) -> Vec2 {
    let dx = b.x - a.x;
    if dx.abs() <= f32::EPSILON {
        return Vec2::new(x, a.y);
    }
    let t = (x - a.x) / dx;
    Vec2::new(x, a.y + t * (b.y - a.y))
}

/// Returns intersection point between ab and the horizontal line at y.
fn intersect_horizontal(a: Vec2, b: Vec2, y: f32) -> Vec2 {
    let dy = b.y - a.y;
    if dy.abs() <= f32::EPSILON {
        return Vec2::new(a.x, y);
    }
    let t = (y - a.y) / dy;
    Vec2::new(a.x + t * (b.x - a.x), y)
}
