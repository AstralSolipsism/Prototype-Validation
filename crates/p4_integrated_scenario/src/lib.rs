#![forbid(unsafe_code)]

use integrated_world_core::{IntegratedWorld, IntegratedWorldError, compile_integrated_world};
use p4_world_scenario::{generate_baseline, generate_parity_order, generate_reverse_order};

pub fn generate_integrated_baseline() -> Result<IntegratedWorld, IntegratedWorldError> {
    let base = generate_baseline().expect("P4A baseline remains valid");
    compile_integrated_world(&base)
}

pub fn generate_integrated_reverse() -> Result<IntegratedWorld, IntegratedWorldError> {
    let base = generate_reverse_order().expect("P4A reverse baseline remains valid");
    compile_integrated_world(&base)
}

pub fn generate_integrated_parity() -> Result<IntegratedWorld, IntegratedWorldError> {
    let base = generate_parity_order().expect("P4A parity baseline remains valid");
    compile_integrated_world(&base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_contains_every_internal_work_package() {
        let world = generate_integrated_baseline().expect("integrated world");
        assert_eq!(world.atlas.cells.len(), 19);
        assert!(world.detailed.cells.len() >= 7);
        assert!(world.history.events.len() >= 5);
        assert_eq!(world.traversal.routes.len(), 3);
        assert!(world.report.all_passed());
    }

    #[test]
    fn scenario_is_invariant_to_p4a_cell_iteration_order() {
        let canonical = generate_integrated_baseline().expect("canonical");
        let reverse = generate_integrated_reverse().expect("reverse");
        let parity = generate_integrated_parity().expect("parity");
        assert_eq!(canonical, reverse);
        assert_eq!(canonical, parity);
    }
}
