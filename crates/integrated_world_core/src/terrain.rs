use crate::atlas::{
    classify_landform, detailed_height, detailed_slope, digest, nearest_cell, point_in_hex,
    river_center_z,
};
use crate::model::*;
use glam::{DVec2, DVec3};
use std::collections::BTreeSet;
use world_generation_core::{Biome, HexCoord};

pub fn default_materialized_cells(atlas: &WorldAtlasManifest) -> Vec<HexCoord> {
    let requested = [
        HexCoord::new(-2, 0),
        HexCoord::new(-1, 0),
        HexCoord::new(-1, 1),
        HexCoord::new(0, -1),
        HexCoord::new(0, 0),
        HexCoord::new(0, 1),
        HexCoord::new(1, -1),
        HexCoord::new(1, 0),
        HexCoord::new(2, 0),
    ];
    requested
        .into_iter()
        .filter(|coord| atlas.cell(*coord).is_some())
        .collect()
}

pub fn materialize_region(
    atlas: &WorldAtlasManifest,
    materialized_cells: &[HexCoord],
) -> Result<DetailedRegion, serde_json::Error> {
    let selected = materialized_cells.iter().copied().collect::<BTreeSet<_>>();
    let (origin_xz, maximum_xz) = region_bounds(atlas, &selected);
    let resolution = REGION_TERRAIN_RESOLUTION;
    let width = usize::from(resolution);
    let height = usize::from(resolution);
    let span = maximum_xz - origin_xz;
    let spacing = (span.x / (width - 1) as f64)
        .max(span.y / (height - 1) as f64)
        .max(1.0);

    let mut samples = Vec::with_capacity(width * height);
    for z_index in 0..height {
        for x_index in 0..width {
            let x = origin_xz.x + x_index as f64 * spacing;
            let z = origin_xz.y + z_index as f64 * spacing;
            let xz = DVec2::new(x, z);
            let cell = nearest_cell(atlas, xz).filter(|coord| selected.contains(coord));
            let elevation = detailed_height(atlas.world_seed, atlas.cell_radius_m, xz);
            let slope = detailed_slope(atlas.world_seed, atlas.cell_radius_m, xz);
            let landform =
                classify_landform(atlas.world_seed, atlas.cell_radius_m, xz, elevation, slope);
            let buildable = cell.is_some()
                && slope <= 0.18
                && elevation > 2.0
                && !matches!(
                    landform,
                    LandformClass::Ocean | LandformClass::Coast | LandformClass::Estuary
                );
            samples.push(TerrainSample {
                grid_x: x_index as u16,
                grid_z: z_index as u16,
                world_position: DVec3::new(x, elevation, z),
                slope,
                flow_accumulation: if cell.is_some() { 1.0 } else { 0.0 },
                landform,
                land_cover: natural_cover(atlas, cell, landform, elevation),
                travel_cost: travel_cost(landform, slope, buildable),
                buildable,
                cell,
            });
        }
    }

    accumulate_flow(width, height, &mut samples);
    let terrain = TerrainGrid {
        width: resolution,
        height: resolution,
        origin_xz,
        spacing_m: spacing,
        samples,
    };

    let mut cells = materialized_cells
        .iter()
        .filter_map(|coord| {
            atlas
                .cell(*coord)
                .map(|spec| materialize_cell(atlas, spec, &terrain))
        })
        .collect::<Result<Vec<_>, _>>()?;
    cells.sort_by_key(|cell| cell.coord);

    let mut region = DetailedRegion {
        materialized_cells: materialized_cells.to_vec(),
        terrain,
        cells,
        semantic_fingerprint: 0,
    };
    region.semantic_fingerprint = digest(&region)?;
    Ok(region)
}

fn materialize_cell(
    atlas: &WorldAtlasManifest,
    spec: &AtlasCellSpec,
    region: &TerrainGrid,
) -> Result<DetailedCell, serde_json::Error> {
    let center = spec.coord.center_xz(atlas.cell_radius_m);
    let radius = atlas.cell_radius_m;
    let width_m = 3.0_f64.sqrt() * radius;
    let resolution = usize::from(CELL_TERRAIN_RESOLUTION);
    let mut samples = Vec::new();

    for z_index in 0..resolution {
        let z = center.y - radius + 2.0 * radius * z_index as f64 / (resolution - 1) as f64;
        for x_index in 0..resolution {
            let x = center.x - width_m * 0.5 + width_m * x_index as f64 / (resolution - 1) as f64;
            let xz = DVec2::new(x, z);
            if !point_in_hex(xz, center, radius) {
                continue;
            }
            let elevation = detailed_height(atlas.world_seed, radius, xz);
            let slope = detailed_slope(atlas.world_seed, radius, xz);
            let landform = classify_landform(atlas.world_seed, radius, xz, elevation, slope);
            let buildable = slope <= 0.18
                && elevation > 2.0
                && !matches!(
                    landform,
                    LandformClass::Ocean | LandformClass::Coast | LandformClass::Estuary
                );
            let flow = region_flow_at(region, xz);
            samples.push(TerrainSample {
                grid_x: x_index as u16,
                grid_z: z_index as u16,
                world_position: DVec3::new(x, elevation, z),
                slope,
                flow_accumulation: flow,
                landform,
                land_cover: natural_cover(atlas, Some(spec.coord), landform, elevation),
                travel_cost: travel_cost(landform, slope, buildable),
                buildable,
                cell: Some(spec.coord),
            });
        }
    }

    let edge_profiles = spec
        .boundary_contract_ids
        .iter()
        .filter_map(|id| atlas.contract(*id))
        .map(|contract| CellEdgeProfile {
            contract_id: contract.id,
            boundary: contract.boundary,
            samples: contract.samples.clone(),
        })
        .collect::<Vec<_>>();
    let (recomputed_elevation, recomputed_landforms) = summarize_samples(&samples);
    let semantic_fingerprint = digest(&(
        spec.id,
        spec.coord,
        CELL_TERRAIN_RESOLUTION,
        &samples,
        &edge_profiles,
    ))?;
    Ok(DetailedCell {
        atlas_cell_id: spec.id,
        coord: spec.coord,
        resolution: CELL_TERRAIN_RESOLUTION,
        samples,
        edge_profiles,
        recomputed_elevation,
        recomputed_landforms,
        semantic_fingerprint,
    })
}

fn region_bounds(atlas: &WorldAtlasManifest, selected: &BTreeSet<HexCoord>) -> (DVec2, DVec2) {
    let half_width = 3.0_f64.sqrt() * atlas.cell_radius_m * 0.5;
    let mut minimum = DVec2::splat(f64::INFINITY);
    let mut maximum = DVec2::splat(f64::NEG_INFINITY);
    for coord in selected {
        let center = coord.center_xz(atlas.cell_radius_m);
        minimum = minimum.min(center - DVec2::new(half_width, atlas.cell_radius_m));
        maximum = maximum.max(center + DVec2::new(half_width, atlas.cell_radius_m));
    }
    (minimum, maximum)
}

fn natural_cover(
    atlas: &WorldAtlasManifest,
    cell: Option<HexCoord>,
    landform: LandformClass,
    elevation: f64,
) -> LandCover {
    match landform {
        LandformClass::Ocean | LandformClass::Estuary => LandCover::OpenWater,
        LandformClass::Coast | LandformClass::Floodplain => LandCover::Wetland,
        LandformClass::Mountain | LandformClass::Ridge if elevation > 165.0 => LandCover::BareRock,
        LandformClass::Hillslope | LandformClass::Mountain => LandCover::Scrub,
        LandformClass::Valley | LandformClass::Lowland | LandformClass::Terrace => cell
            .and_then(|coord| atlas.cell(coord))
            .map(|spec| match spec.biome {
                Biome::TemperateForest => LandCover::Forest,
                Biome::Wetland => LandCover::Wetland,
                _ => LandCover::Grassland,
            })
            .unwrap_or(LandCover::Grassland),
        LandformClass::Ridge => LandCover::Scrub,
    }
}

fn travel_cost(landform: LandformClass, slope: f64, buildable: bool) -> f64 {
    let base = match landform {
        LandformClass::Ocean => 1000.0,
        LandformClass::Estuary => 80.0,
        LandformClass::Coast => 12.0,
        LandformClass::Floodplain => 2.2,
        LandformClass::Valley => 1.2,
        LandformClass::Lowland => 1.0,
        LandformClass::Terrace => 1.1,
        LandformClass::Hillslope => 2.4,
        LandformClass::Ridge => 3.1,
        LandformClass::Mountain => 5.0,
    };
    base + slope.powi(2) * 22.0 + if buildable { 0.0 } else { 0.4 }
}

fn accumulate_flow(width: usize, height: usize, samples: &mut [TerrainSample]) {
    let mut order = (0..samples.len())
        .filter(|index| samples[*index].cell.is_some())
        .collect::<Vec<_>>();
    order.sort_by(|left, right| {
        samples[*right]
            .world_position
            .y
            .total_cmp(&samples[*left].world_position.y)
            .then_with(|| left.cmp(right))
    });

    for index in order {
        let x = index % width;
        let z = index / width;
        let current_elevation = samples[index].world_position.y;
        let mut downstream: Option<usize> = None;
        for dz in -1isize..=1 {
            for dx in -1isize..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let nx = x as isize + dx;
                let nz = z as isize + dz;
                if nx < 0 || nz < 0 || nx >= width as isize || nz >= height as isize {
                    continue;
                }
                let neighbor = nz as usize * width + nx as usize;
                if samples[neighbor].cell.is_none()
                    || samples[neighbor].world_position.y >= current_elevation
                {
                    continue;
                }
                downstream = match downstream {
                    None => Some(neighbor),
                    Some(existing) => {
                        if samples[neighbor].world_position.y < samples[existing].world_position.y {
                            Some(neighbor)
                        } else {
                            Some(existing)
                        }
                    }
                };
            }
        }
        if let Some(downstream) = downstream {
            samples[downstream].flow_accumulation += samples[index].flow_accumulation;
        }
    }

    let maximum = samples
        .iter()
        .map(|sample| sample.flow_accumulation)
        .fold(1.0_f64, f64::max)
        .ln_1p();
    for sample in samples.iter_mut().filter(|sample| sample.cell.is_some()) {
        sample.flow_accumulation = sample.flow_accumulation.ln_1p() / maximum;
        let distance_to_river =
            (sample.world_position.z - river_center_z(sample.world_position.x, 128.0)).abs();
        if sample.flow_accumulation > 0.72
            && distance_to_river < 38.0
            && sample.world_position.y > 0.0
        {
            sample.landform = if sample.world_position.y < 8.0 {
                LandformClass::Estuary
            } else {
                LandformClass::Floodplain
            };
        }
    }
}

fn region_flow_at(region: &TerrainGrid, point: DVec2) -> f64 {
    let x = ((point.x - region.origin_xz.x) / region.spacing_m)
        .round()
        .clamp(0.0, f64::from(region.width - 1)) as usize;
    let z = ((point.y - region.origin_xz.y) / region.spacing_m)
        .round()
        .clamp(0.0, f64::from(region.height - 1)) as usize;
    region
        .sample(x, z)
        .map(|sample| sample.flow_accumulation)
        .unwrap_or(0.0)
}

pub fn summarize_samples(samples: &[TerrainSample]) -> (ElevationSummary, LandformMix) {
    let mut elevations = samples
        .iter()
        .map(|sample| sample.world_position.y)
        .collect::<Vec<_>>();
    elevations.sort_by(f64::total_cmp);
    let count = elevations.len().max(1);
    let minimum = elevations.first().copied().unwrap_or(0.0);
    let maximum = elevations.last().copied().unwrap_or(0.0);
    let mean = elevations.iter().sum::<f64>() / count as f64;
    let median = elevations.get(count / 2).copied().unwrap_or(mean);
    let mean_slope = samples.iter().map(|sample| sample.slope).sum::<f64>() / count as f64;
    let buildable = samples.iter().filter(|sample| sample.buildable).count() as f64 / count as f64;
    let mut buckets = [0usize; 7];
    for sample in samples {
        let bucket = match sample.landform {
            LandformClass::Mountain => 0,
            LandformClass::Ridge => 1,
            LandformClass::Hillslope | LandformClass::Terrace => 2,
            LandformClass::Valley => 3,
            LandformClass::Lowland | LandformClass::Floodplain => 4,
            LandformClass::Coast | LandformClass::Estuary => 5,
            LandformClass::Ocean => 6,
        };
        buckets[bucket] += 1;
    }
    let fraction = |bucket: usize| buckets[bucket] as f64 / count as f64;
    (
        ElevationSummary {
            minimum_m: minimum,
            maximum_m: maximum,
            mean_m: mean,
            median_m: median,
            relief_m: maximum - minimum,
            mean_slope,
            buildable_fraction: buildable,
        },
        LandformMix {
            mountain: fraction(0),
            ridge: fraction(1),
            hillslope: fraction(2),
            valley: fraction(3),
            lowland: fraction(4),
            coast: fraction(5),
            water: fraction(6),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::build_atlas;
    use world_generation_core::{
        GeneratorVersion, TraversalOrder, WorldGenerationConfig, generate_world_with_order,
    };
    use world_ids::{BuildingId, WorldId};

    fn fixture_atlas() -> WorldAtlasManifest {
        let base = generate_world_with_order(
            WorldGenerationConfig {
                world_id: WorldId::from_u128(0x5044),
                world_seed: 0x706f_7274_2d76_616c_6c65_792d_3031,
                generator_version: GeneratorVersion(1),
                radius: 2,
                cell_radius_m: 128.0,
                building_blueprint_id: BuildingId::from_u128(1),
            },
            TraversalOrder::Canonical,
        )
        .expect("base world");
        build_atlas(&base).expect("atlas")
    }

    #[test]
    fn materializes_a_mountain_to_ocean_region() {
        let atlas = fixture_atlas();
        let coords = default_materialized_cells(&atlas);
        let region = materialize_region(&atlas, &coords).expect("region");
        assert!(region.cells.len() >= 7);
        assert!(
            region
                .terrain
                .samples
                .iter()
                .any(|sample| sample.landform == LandformClass::Mountain)
        );
        assert!(
            region
                .terrain
                .samples
                .iter()
                .any(|sample| sample.landform == LandformClass::Valley)
        );
        assert!(
            region
                .terrain
                .samples
                .iter()
                .any(|sample| sample.landform == LandformClass::Ocean)
        );
        assert!(
            region
                .terrain
                .samples
                .iter()
                .any(|sample| sample.landform == LandformClass::Coast)
        );
    }

    #[test]
    fn adjacent_cells_reuse_the_exact_same_edge_contract() {
        let atlas = fixture_atlas();
        let coords = default_materialized_cells(&atlas);
        let region = materialize_region(&atlas, &coords).expect("region");
        let left = region.cell(HexCoord::new(0, 0)).expect("center");
        let right = region.cell(HexCoord::new(1, 0)).expect("east");
        let shared = left
            .edge_profiles
            .iter()
            .find(|profile| profile.boundary.contains(right.coord))
            .expect("shared profile");
        let other = right
            .edge_profiles
            .iter()
            .find(|profile| profile.contract_id == shared.contract_id)
            .expect("same profile");
        assert_eq!(shared.samples, other.samples);
    }

    #[test]
    fn detailed_summary_matches_atlas_within_sampling_tolerance() {
        let atlas = fixture_atlas();
        let coords = default_materialized_cells(&atlas);
        let region = materialize_region(&atlas, &coords).expect("region");
        for cell in &region.cells {
            let atlas_cell = atlas.cell(cell.coord).expect("atlas cell");
            assert!((atlas_cell.elevation.mean_m - cell.recomputed_elevation.mean_m).abs() < 12.0);
            assert!(
                (atlas_cell.elevation.relief_m - cell.recomputed_elevation.relief_m).abs() < 24.0
            );
            assert!((cell.recomputed_landforms.total() - 1.0).abs() < 1.0e-9);
        }
    }
}
