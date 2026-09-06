#![forbid(unsafe_code)]

use region_scale_core::{RegionScaleError, RegionScaleWorld, build_world};

pub fn generate_region_scale_world() -> Result<RegionScaleWorld, RegionScaleError> {
    build_world()
}

#[cfg(test)]
mod tests {
    use super::*;
    use region_scale_core::{CELL_FLAT_TO_FLAT_M, validate_world};

    #[test]
    fn scenario_has_region_scale_cells_and_complete_validation() {
        let world = generate_region_scale_world().expect("world");
        assert_eq!(world.atlas.cells.len(), 19);
        assert!((world.atlas.flat_to_flat_m - CELL_FLAT_TO_FLAT_M).abs() < 1.0e-9);
        let report = validate_world(&world).expect("report");
        assert!(report.all_passed());
    }
}
