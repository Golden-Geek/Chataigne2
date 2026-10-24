use super::*;

#[cfg(test)]
pub(super) fn spatializer_value(
    config: &SpatializerConfig,
    target: &SpatializerEndpointConfig,
    source: &SpatializerEndpointConfig,
) -> f64 {
    value_for_pair(&spatializer_values_by_target(config), target.item_id, source.item_id)
}

/// Source-major output storage, admitted before allocation. No pair-sized maps
/// or actor-side matrix calculation are needed by the runtime.
pub(super) fn dense_values(config: &SpatializerConfig, cancelled: impl Fn() -> bool) -> Option<Vec<f64>> {
    let mut values = vec![0.0; config.sources.len().checked_mul(config.targets.len())?];
    let topology = (config.mode == SpatializerMode::Voronoi).then(|| VoronoiTopology::compile(config));
    let indices: HashMap<_, _> = config.targets.iter().enumerate().map(|(i, t)| (t.item_id, i)).collect();
    for (source_index, source) in config.sources.iter().enumerate() {
        if cancelled() {
            return None;
        }
        if !source.enabled {
            continue;
        }
        if let Some(topology) = &topology {
            for (id, value) in voronoi_target_values_for_source(config, source, topology) {
                values[source_index * config.targets.len() + indices[&id]] = value;
            }
        } else {
            for (target_index, target) in config.targets.iter().enumerate() {
                if target_index % 128 == 0 && cancelled() {
                    return None;
                }
                if !target.enabled {
                    continue;
                }
                let distance = source.position.distance_to(target.position, config.dimension);
                let value = match config.mode {
                    SpatializerMode::TargetRadius => radius_influence(distance, target.radius),
                    SpatializerMode::SourceRadius => radius_influence(distance, source.radius),
                    SpatializerMode::Overlap => overlap_value(distance, source.radius, target.radius),
                    SpatializerMode::Voronoi => unreachable!(),
                };
                values[source_index * config.targets.len() + target_index] = value;
            }
        }
    }
    Some(values)
}

#[cfg(test)]
pub(super) fn spatializer_values_by_target(config: &SpatializerConfig) -> HashMap<NodeId, HashMap<NodeId, f64>> {
    let values = dense_values(config, || false).expect("test configuration");
    config
        .targets
        .iter()
        .enumerate()
        .map(|(t, target)| {
            (
                target.item_id,
                config
                    .sources
                    .iter()
                    .enumerate()
                    .map(|(s, source)| (source.item_id, values[s * config.targets.len() + t]))
                    .collect(),
            )
        })
        .collect()
}

#[cfg(test)]
fn value_for_pair(values: &HashMap<NodeId, HashMap<NodeId, f64>>, target: NodeId, source: NodeId) -> f64 {
    values
        .get(&target)
        .and_then(|row| row.get(&source))
        .copied()
        .unwrap_or(0.0)
}

pub(super) fn voronoi_target_values_for_source(
    config: &SpatializerConfig,
    source: &SpatializerEndpointConfig,
    topology: &VoronoiTopology,
) -> HashMap<NodeId, f64> {
    let target_indices = topology.target_indices.as_slice();
    if target_indices.is_empty() {
        return HashMap::new();
    }

    let frozen_indices = frozen_target_indices(config, source.position, target_indices);
    if !frozen_indices.is_empty() {
        return split_between_targets(config, &frozen_indices);
    }

    let Some(current_index) = nearest_target_index(config, source.position, target_indices) else {
        return HashMap::new();
    };
    let current = &config.targets[current_index];
    let current_distance = current.position.distance_to(source.position, config.dimension);
    let current_distance = (current_distance - current.freeze_radius.max(0.0)).max(0.0);
    if current_distance <= VORONOI_TIE_EPSILON {
        return split_between_targets(config, &[current_index]);
    }

    let mut raw_values = vec![(current.item_id, morpher_raw_weight_from_distance(current_distance))];
    let mut boundary_entries = if config.dimension == SpatializerDimension::Two {
        morpher_boundary_entries_2d(
            config,
            source.position,
            current_index,
            target_indices,
            topology.neighbours(current_index),
        )
    } else {
        morpher_boundary_entries_nd(config, source.position, current_index, target_indices)
    };
    if boundary_entries.is_empty() && target_indices.len() > 1 {
        boundary_entries = morpher_boundary_entries_nd(config, source.position, current_index, target_indices);
    }

    let mut smallest_edge = (usize::MAX, f64::INFINITY);
    let mut second_smallest_edge = f64::INFINITY;
    for (index, entry) in boundary_entries.iter().enumerate() {
        if entry.edge_distance < smallest_edge.1 {
            second_smallest_edge = smallest_edge.1;
            smallest_edge = (index, entry.edge_distance);
        } else if entry.edge_distance < second_smallest_edge {
            second_smallest_edge = entry.edge_distance;
        }
    }
    for entry_index in 0..boundary_entries.len() {
        let entry = boundary_entries[entry_index];
        let weight = if boundary_entries.len() > 1 {
            let min_other_edge_distance = if entry_index == smallest_edge.0 {
                second_smallest_edge
            } else {
                smallest_edge.1
            };
            let denominator = entry.edge_distance + min_other_edge_distance;
            let ratio = if denominator <= VORONOI_TIE_EPSILON {
                1.0
            } else {
                1.0 - entry.edge_distance / denominator
            }
            .clamp(0.0, 1.0);
            morpher_raw_weight_from_path(ratio, entry.edge_distance + entry.neighbour_distance)
        } else {
            let neighbour = &config.targets[entry.neighbour_index];
            let direct_distance = neighbour.position.distance_to(source.position, config.dimension);
            morpher_raw_weight_from_distance((direct_distance - neighbour.freeze_radius.max(0.0)).max(0.0))
        };
        if weight > 0.0 {
            raw_values.push((config.targets[entry.neighbour_index].item_id, weight));
        }
    }

    normalize_raw_values(raw_values)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VoronoiBoundaryEntry {
    neighbour_index: usize,
    edge_distance: f64,
    neighbour_distance: f64,
}

#[derive(Debug)]
pub(super) struct VoronoiTopology {
    pub(super) target_indices: Vec<usize>,
    neighbours_by_target: Vec<Vec<usize>>,
}

impl VoronoiTopology {
    pub(super) fn compile(config: &SpatializerConfig) -> Self {
        let target_indices = enabled_target_indices(config);
        let mut neighbours_by_target = vec![Vec::new(); config.targets.len()];
        if target_indices.len() < 2 {
            return Self {
                target_indices,
                neighbours_by_target,
            };
        }

        if target_indices.len() == 2 {
            connect_topology_targets(&mut neighbours_by_target, target_indices[0], target_indices[1]);
        } else {
            let points: Vec<Point> = target_indices
                .iter()
                .map(|target_index| {
                    let position = config.targets[*target_index].position;
                    Point {
                        x: position.x,
                        y: position.y,
                    }
                })
                .collect();
            let triangulation = triangulate(&points);
            for triangle in triangulation.triangles.chunks_exact(3) {
                let first = target_indices[triangle[0]];
                let second = target_indices[triangle[1]];
                let third = target_indices[triangle[2]];
                connect_topology_targets(&mut neighbours_by_target, first, second);
                connect_topology_targets(&mut neighbours_by_target, second, third);
                connect_topology_targets(&mut neighbours_by_target, third, first);
            }

            if triangulation.is_empty() {
                connect_collinear_topology(config, &target_indices, &mut neighbours_by_target);
            }
        }

        // Delaunator deliberately omits duplicate points. Keep those valid product inputs
        // connected to their nearest site without falling back to an all-pairs topology.
        for target_index in target_indices.iter().copied() {
            if neighbours_by_target[target_index].is_empty() {
                let neighbour_index = target_indices
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate != target_index)
                    .min_by(|left, right| {
                        let position = config.targets[target_index].position;
                        position
                            .distance_to(config.targets[*left].position, SpatializerDimension::Two)
                            .total_cmp(
                                &position.distance_to(config.targets[*right].position, SpatializerDimension::Two),
                            )
                    });
                if let Some(neighbour_index) = neighbour_index {
                    connect_topology_targets(&mut neighbours_by_target, target_index, neighbour_index);
                }
            }
        }
        for neighbours in &mut neighbours_by_target {
            neighbours.sort_unstable();
            neighbours.dedup();
        }

        Self {
            target_indices,
            neighbours_by_target,
        }
    }

    pub(super) fn neighbours(&self, target_index: usize) -> &[usize] {
        self.neighbours_by_target
            .get(target_index)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

pub(super) fn connect_topology_targets(neighbours_by_target: &mut [Vec<usize>], first: usize, second: usize) {
    if first == second {
        return;
    }
    neighbours_by_target[first].push(second);
    neighbours_by_target[second].push(first);
}

pub(super) fn connect_collinear_topology(
    config: &SpatializerConfig,
    target_indices: &[usize],
    neighbours_by_target: &mut [Vec<usize>],
) {
    let mut sorted = target_indices.to_vec();
    sorted.sort_by(|left, right| {
        let left_position = config.targets[*left].position;
        let right_position = config.targets[*right].position;
        left_position
            .x
            .total_cmp(&right_position.x)
            .then_with(|| left_position.y.total_cmp(&right_position.y))
            .then_with(|| left.cmp(right))
    });
    for adjacent in sorted.windows(2) {
        connect_topology_targets(neighbours_by_target, adjacent[0], adjacent[1]);
    }
}

pub(super) fn enabled_target_indices(config: &SpatializerConfig) -> Vec<usize> {
    config
        .targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.enabled)
        .map(|(target_index, _)| target_index)
        .collect()
}

pub(super) fn nearest_target_index(
    config: &SpatializerConfig,
    position: SpatialPoint,
    target_indices: &[usize],
) -> Option<usize> {
    target_indices.iter().copied().min_by(|left, right| {
        let left_distance = config.targets[*left].position.distance_to(position, config.dimension);
        let right_distance = config.targets[*right].position.distance_to(position, config.dimension);
        left_distance.total_cmp(&right_distance)
    })
}

pub(super) fn frozen_target_indices(
    config: &SpatializerConfig,
    position: SpatialPoint,
    target_indices: &[usize],
) -> Vec<usize> {
    let frozen_distance = target_indices
        .iter()
        .filter_map(|target_index| {
            let freeze_radius = config.targets[*target_index].freeze_radius.max(0.0);
            let distance = config.targets[*target_index]
                .position
                .distance_to(position, config.dimension);
            (distance <= freeze_radius + VORONOI_TIE_EPSILON).then_some(distance)
        })
        .min_by(f64::total_cmp);
    let Some(frozen_distance) = frozen_distance else {
        return Vec::new();
    };

    target_indices
        .iter()
        .filter_map(|target_index| {
            let distance = config.targets[*target_index]
                .position
                .distance_to(position, config.dimension);
            distances_tie(distance, frozen_distance).then_some(*target_index)
        })
        .collect()
}

pub(super) fn split_between_targets(config: &SpatializerConfig, target_indices: &[usize]) -> HashMap<NodeId, f64> {
    if target_indices.is_empty() {
        return HashMap::new();
    }

    let value = 1.0 / target_indices.len() as f64;
    target_indices
        .iter()
        .map(|target_index| (config.targets[*target_index].item_id, value))
        .collect()
}

pub(super) fn morpher_boundary_entries_2d(
    config: &SpatializerConfig,
    position: SpatialPoint,
    current_index: usize,
    target_indices: &[usize],
    neighbour_indices: &[usize],
) -> Vec<VoronoiBoundaryEntry> {
    let current_cell = target_voronoi_cell_polygon(config, current_index, position, target_indices, neighbour_indices);
    if current_cell.len() < 3 {
        return Vec::new();
    }

    neighbour_indices
        .iter()
        .copied()
        .filter(|target_index| *target_index != current_index)
        .filter_map(|neighbour_index| {
            closest_cell_edge_for_neighbour(config, position, &current_cell, current_index, neighbour_index)
        })
        .collect()
}

pub(super) fn morpher_boundary_entries_nd(
    config: &SpatializerConfig,
    position: SpatialPoint,
    current_index: usize,
    target_indices: &[usize],
) -> Vec<VoronoiBoundaryEntry> {
    target_indices
        .iter()
        .copied()
        .filter(|target_index| *target_index != current_index)
        .filter_map(|neighbour_index| boundary_entry_for_bisector(config, position, current_index, neighbour_index))
        .collect()
}

pub(super) fn target_voronoi_cell_polygon(
    config: &SpatializerConfig,
    current_index: usize,
    position: SpatialPoint,
    target_indices: &[usize],
    neighbour_indices: &[usize],
) -> Vec<SpatialPoint> {
    let current = config.targets[current_index].position;
    let mut polygon = voronoi_bounds_polygon(config, position, target_indices);
    for other_index in neighbour_indices {
        polygon = clip_to_closer_site(polygon, current, config.targets[*other_index].position);
        if polygon.len() < 3 {
            break;
        }
    }
    polygon
}

pub(super) fn closest_cell_edge_for_neighbour(
    config: &SpatializerConfig,
    position: SpatialPoint,
    cell: &[SpatialPoint],
    current_index: usize,
    neighbour_index: usize,
) -> Option<VoronoiBoundaryEntry> {
    let current = config.targets[current_index].position;
    let neighbour = config.targets[neighbour_index].position;
    let mut best: Option<(f64, SpatialPoint)> = None;
    for edge_index in 0..cell.len() {
        let start = cell[edge_index];
        let end = cell[(edge_index + 1) % cell.len()];
        let start_value = closer_half_plane_value(start, current, neighbour).abs();
        let end_value = closer_half_plane_value(end, current, neighbour).abs();
        if start_value > 1.0e-6 || end_value > 1.0e-6 {
            continue;
        }
        let closest = closest_point_on_segment_2d(position, start, end);
        let distance = position.distance_to(closest, SpatializerDimension::Two);
        if best.as_ref().is_none_or(|(best_distance, _)| distance < *best_distance) {
            best = Some((distance, closest));
        }
    }

    let (edge_distance, closest) = best?;
    let neighbour_distance = (closest.distance_to(neighbour, SpatializerDimension::Two)
        - config.targets[neighbour_index].freeze_radius.max(0.0))
    .max(0.0);
    Some(VoronoiBoundaryEntry {
        neighbour_index,
        edge_distance,
        neighbour_distance,
    })
}

pub(super) fn boundary_entry_for_bisector(
    config: &SpatializerConfig,
    position: SpatialPoint,
    current_index: usize,
    neighbour_index: usize,
) -> Option<VoronoiBoundaryEntry> {
    let current = config.targets[current_index].position;
    let neighbour = config.targets[neighbour_index].position;
    let separation = current.distance_to(neighbour, config.dimension);
    if !separation.is_finite() || separation <= VORONOI_TIE_EPSILON {
        return None;
    }

    let unit = normalized_direction(current, neighbour, config.dimension)?;
    let midpoint = midpoint_for_dimension(current, neighbour, config.dimension);
    let signed_distance = dot_for_dimension(
        SpatialPoint::new(
            position.x - midpoint.x,
            position.y - midpoint.y,
            position.z - midpoint.z,
        ),
        unit,
        config.dimension,
    );
    let closest = SpatialPoint::new(
        position.x - unit.x * signed_distance,
        position.y - unit.y * signed_distance,
        if config.dimension == SpatializerDimension::Three {
            position.z - unit.z * signed_distance
        } else {
            0.0
        },
    );
    let neighbour_distance = (closest.distance_to(neighbour, config.dimension)
        - config.targets[neighbour_index].freeze_radius.max(0.0))
    .max(0.0);
    Some(VoronoiBoundaryEntry {
        neighbour_index,
        edge_distance: signed_distance.abs(),
        neighbour_distance,
    })
}

pub(super) fn normalized_direction(
    from: SpatialPoint,
    to: SpatialPoint,
    dimension: SpatializerDimension,
) -> Option<SpatialPoint> {
    let distance = from.distance_to(to, dimension);
    if !distance.is_finite() || distance <= VORONOI_TIE_EPSILON {
        return None;
    }
    Some(SpatialPoint::new(
        (to.x - from.x) / distance,
        (to.y - from.y) / distance,
        if dimension == SpatializerDimension::Three {
            (to.z - from.z) / distance
        } else {
            0.0
        },
    ))
}

pub(super) fn midpoint_for_dimension(
    left: SpatialPoint,
    right: SpatialPoint,
    dimension: SpatializerDimension,
) -> SpatialPoint {
    SpatialPoint::new(
        (left.x + right.x) * 0.5,
        (left.y + right.y) * 0.5,
        if dimension == SpatializerDimension::Three {
            (left.z + right.z) * 0.5
        } else {
            0.0
        },
    )
}

pub(super) fn dot_for_dimension(left: SpatialPoint, right: SpatialPoint, dimension: SpatializerDimension) -> f64 {
    left.x * right.x
        + left.y * right.y
        + if dimension == SpatializerDimension::Three {
            left.z * right.z
        } else {
            0.0
        }
}

pub(super) fn closest_point_on_segment_2d(point: SpatialPoint, start: SpatialPoint, end: SpatialPoint) -> SpatialPoint {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= VORONOI_TIE_EPSILON {
        return start;
    }
    let t = (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    SpatialPoint::new(start.x + dx * t, start.y + dy * t, 0.0)
}

pub(super) fn morpher_raw_weight_from_distance(distance: f64) -> f64 {
    if !distance.is_finite() {
        return 0.0;
    }
    if distance <= VORONOI_TIE_EPSILON {
        return 1.0e12;
    }
    1.0 / distance
}

pub(super) fn morpher_raw_weight_from_path(ratio: f64, distance: f64) -> f64 {
    let ratio = ratio.clamp(0.0, 1.0);
    if ratio <= 0.0 || !distance.is_finite() {
        return 0.0;
    }
    if distance <= VORONOI_TIE_EPSILON {
        return 1.0e12;
    }
    ratio / distance
}

pub(super) fn voronoi_bounds_polygon(
    config: &SpatializerConfig,
    position: SpatialPoint,
    target_indices: &[usize],
) -> Vec<SpatialPoint> {
    let mut left = position.x;
    let mut right = position.x;
    let mut top = position.y;
    let mut bottom = position.y;
    for target_index in target_indices {
        let position = config.targets[*target_index].position;
        left = left.min(position.x);
        right = right.max(position.x);
        top = top.min(position.y);
        bottom = bottom.max(position.y);
    }

    let width = (right - left).abs();
    let height = (bottom - top).abs();
    let padding = width.max(height).max(1.0) * 4.0;
    left -= padding;
    right += padding;
    top -= padding;
    bottom += padding;

    vec![
        SpatialPoint::new(left, top, 0.0),
        SpatialPoint::new(right, top, 0.0),
        SpatialPoint::new(right, bottom, 0.0),
        SpatialPoint::new(left, bottom, 0.0),
    ]
}

pub(super) fn clip_to_closer_site(
    polygon: Vec<SpatialPoint>,
    site: SpatialPoint,
    other: SpatialPoint,
) -> Vec<SpatialPoint> {
    if polygon.is_empty() {
        return polygon;
    }

    let mut clipped = Vec::new();
    for index in 0..polygon.len() {
        let current = polygon[index];
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let current_value = closer_half_plane_value(current, site, other);
        let previous_value = closer_half_plane_value(previous, site, other);
        let current_inside = current_value <= VORONOI_TIE_EPSILON;
        let previous_inside = previous_value <= VORONOI_TIE_EPSILON;

        if current_inside && !previous_inside {
            clipped.push(half_plane_intersection(
                previous,
                current,
                previous_value,
                current_value,
            ));
        }
        if current_inside {
            clipped.push(current);
        } else if previous_inside {
            clipped.push(half_plane_intersection(
                previous,
                current,
                previous_value,
                current_value,
            ));
        }
    }
    clipped
}

pub(super) fn closer_half_plane_value(point: SpatialPoint, site: SpatialPoint, other: SpatialPoint) -> f64 {
    let dx = other.x - site.x;
    let dy = other.y - site.y;
    2.0 * (dx * point.x + dy * point.y) - (other.x * other.x + other.y * other.y - site.x * site.x - site.y * site.y)
}

pub(super) fn half_plane_intersection(
    start: SpatialPoint,
    end: SpatialPoint,
    start_value: f64,
    end_value: f64,
) -> SpatialPoint {
    let denominator = start_value - end_value;
    if denominator.abs() <= VORONOI_TIE_EPSILON {
        return end;
    }
    let t = (start_value / denominator).clamp(0.0, 1.0);
    SpatialPoint::new(start.x + (end.x - start.x) * t, start.y + (end.y - start.y) * t, 0.0)
}

pub(super) fn normalize_raw_values(raw_values: Vec<(NodeId, f64)>) -> HashMap<NodeId, f64> {
    let total: f64 = raw_values
        .iter()
        .map(|(_, value)| *value)
        .filter(|value| value.is_finite() && *value > 0.0)
        .sum();
    if total <= 0.0 {
        return HashMap::new();
    }

    raw_values
        .into_iter()
        .filter(|(_, value)| value.is_finite() && *value > 0.0)
        .map(|(source_id, value)| (source_id, clamp01(value / total)))
        .collect()
}

pub(super) fn distances_tie(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= VORONOI_TIE_EPSILON
}

pub(super) fn radius_influence(distance: f64, radius: f64) -> f64 {
    if !distance.is_finite() || !radius.is_finite() {
        return 0.0;
    }
    if radius <= 0.0 {
        return if distance <= 0.0 { 1.0 } else { 0.0 };
    }
    clamp01(1.0 - distance / radius)
}

pub(super) fn overlap_value(distance: f64, source_radius: f64, target_radius: f64) -> f64 {
    if !distance.is_finite() || !source_radius.is_finite() || !target_radius.is_finite() {
        return 0.0;
    }
    let source_radius = source_radius.max(0.0);
    let target_radius = target_radius.max(0.0);
    if source_radius <= 0.0 || target_radius <= 0.0 {
        return if distance <= 0.0 && source_radius == target_radius {
            1.0
        } else {
            0.0
        };
    }

    let min_radius = source_radius.min(target_radius);
    let max_radius = source_radius.max(target_radius);
    if distance <= max_radius - min_radius {
        return 1.0;
    }
    let separation = source_radius + target_radius;
    if distance >= separation {
        return 0.0;
    }

    clamp01((separation - distance) / (2.0 * min_radius))
}

pub(super) fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}
