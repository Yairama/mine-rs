//! Tests de integración mínimos para la fachada pública de `mine-sdk`.

use mine_sdk::{experimental, public_layers, recommended_modules};

#[test]
fn include_core_and_sdk_layers() {
    let layers = public_layers();

    assert_eq!(layers[0].name, "mine-core");
    assert_eq!(layers[1].name, "mine-sdk");
    assert!(layers[1].responsibility.contains("API publica"));
}

#[test]
fn expose_experimental_namespace_for_opt_in_prototypes() {
    let pushback_plan = std::any::type_name::<experimental::PushbackPlan>();
    let adaptive_reblock = std::any::type_name::<experimental::AdaptiveReblockPrototype>();

    assert!(pushback_plan.contains("PushbackPlan"));
    assert!(adaptive_reblock.contains("AdaptiveReblockPrototype"));
}

#[test]
fn describe_recommended_domain_modules() {
    let modules = recommended_modules();
    let names = modules.map(|module| module.name);

    assert_eq!(names[0], "mine_sdk::core");
    assert!(names.contains(&"mine_sdk::blockmodel"));
    assert!(names.contains(&"mine_sdk::planning"));
    assert!(names.contains(&"mine_sdk::experimental"));
    assert!(
        modules
            .iter()
            .any(|module| module.responsibility.contains("Prototipos opt-in"))
    );
}
