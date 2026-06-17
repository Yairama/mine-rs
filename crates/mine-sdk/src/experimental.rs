//! Superficie experimental opt-in para `mine-sdk`.
//!
//! La raiz `mine_sdk` sigue siendo la entrada recomendada del SDK alpha. Este
//! modulo agrupa prototipos y rutas de planning/reblocking cuya madurez todavia
//! no debe leerse como contrato publico recomendado.
//!
//! Nota: `ExperimentalVariogram*` no vive aqui. En ese caso "experimental" es
//! terminologia geostatistica del dominio, no un marcador de estabilidad.

pub use mine_planning::{
    PushbackGenerationRules, PushbackPlan, PushbackPrototype, PushbackPrototypeReport,
    UpitPrototypeReport, build_pushback_prototype, build_upit_prototype,
};
pub use mine_reblock::{
    AdaptiveReblockPrototype, AdaptiveResolutionStrategy, AdaptiveZonePrototype, AdaptiveZoneRule,
    build_adaptive_reblock_prototype,
};
