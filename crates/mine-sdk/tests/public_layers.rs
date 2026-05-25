//! Tests de integración mínimos para la fachada pública de `mine-sdk`.

use mine_sdk::public_layers;

#[test]
fn include_core_and_sdk_layers() {
    let layers = public_layers();

    assert_eq!(layers[0].name, "mine-core");
    assert_eq!(layers[1].name, "mine-sdk");
    assert!(layers[1].responsibility.contains("API publica"));
}
