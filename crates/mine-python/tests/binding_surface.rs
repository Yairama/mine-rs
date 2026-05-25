//! Tests de integración mínimos para la superficie pública de `mine-python`.

use _native::binding_surface;

#[test]
fn binding_surface_exposes_sdk_and_tools() {
    let surface = binding_surface();

    assert_eq!(surface.binding_layer, "mine-python");
    assert!(!surface.sdk_layers.is_empty());
    assert!(!surface.available_tools.is_empty());
}
