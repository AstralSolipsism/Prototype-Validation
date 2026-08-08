#![forbid(unsafe_code)]

use building_core::compile_blueprint;
use p3_building_scenario::two_storey_shop;
use world_generation_core::{
    GeneratorVersion, TraversalOrder, WorldCompilation, WorldGenerationConfig,
    WorldGenerationError, generate_world_with_order,
};
use world_ids::WorldId;

pub const P4_WORLD_SEED: u128 = 0x706f_7274_2d76_616c_6c65_792d_3031;

pub fn baseline_config() -> WorldGenerationConfig {
    let blueprint = two_storey_shop();
    compile_blueprint(&blueprint).expect("P3 building blueprint remains valid");
    WorldGenerationConfig {
        world_id: WorldId::from_u128(0x5044),
        world_seed: P4_WORLD_SEED,
        generator_version: GeneratorVersion(1),
        radius: 2,
        cell_radius_m: 128.0,
        building_blueprint_id: blueprint.id,
    }
}

pub fn generate_baseline() -> Result<WorldCompilation, WorldGenerationError> {
    generate_world_with_order(baseline_config(), TraversalOrder::Canonical)
}

pub fn generate_reverse_order() -> Result<WorldCompilation, WorldGenerationError> {
    generate_world_with_order(baseline_config(), TraversalOrder::Reverse)
}

pub fn generate_parity_order() -> Result<WorldCompilation, WorldGenerationError> {
    generate_world_with_order(baseline_config(), TraversalOrder::Parity)
}

pub fn invalid_config() -> WorldGenerationConfig {
    WorldGenerationConfig {
        radius: 0,
        ..baseline_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use world_generation_core::generate_world;

    #[test]
    fn traversal_order_does_not_change_world_semantics() {
        let canonical = generate_baseline().expect("canonical world");
        let reverse = generate_reverse_order().expect("reverse world");
        let parity = generate_parity_order().expect("parity world");
        assert_eq!(canonical.semantic_fingerprint, reverse.semantic_fingerprint);
        assert_eq!(canonical.semantic_fingerprint, parity.semantic_fingerprint);
        assert_eq!(canonical.manifest, reverse.manifest);
        assert_eq!(canonical.manifest, parity.manifest);
    }

    #[test]
    fn baseline_contains_the_required_nineteen_cells() {
        let world = generate_baseline().expect("baseline world");
        assert_eq!(world.manifest.cells.len(), 19);
        assert_eq!(world.manifest.settlements.len(), 1);
        assert_eq!(world.manifest.routes.len(), 3);
        assert!(world.manifest.settlements[0].buildings.len() >= 6);
    }

    #[test]
    fn invalid_generation_config_is_rejected() {
        assert!(generate_world(invalid_config()).is_err());
    }
}
