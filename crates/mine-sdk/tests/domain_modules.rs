//! Tests de integración para los módulos públicos por dominio de `mine-sdk`.

use std::any::TypeId;

#[test]
fn expose_domain_modules_without_breaking_flat_reexports() {
    assert_eq!(
        TypeId::of::<mine_sdk::blockmodel::BlockModel>(),
        TypeId::of::<mine_sdk::BlockModel>()
    );
    assert_eq!(
        TypeId::of::<mine_sdk::validation::ValidationOptions>(),
        TypeId::of::<mine_sdk::ValidationOptions>()
    );
    assert_eq!(
        TypeId::of::<mine_sdk::io::CsvReadOptions>(),
        TypeId::of::<mine_sdk::CsvReadOptions>()
    );
}
