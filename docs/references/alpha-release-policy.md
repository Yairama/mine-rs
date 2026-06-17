# Política de releases alpha `0.x`

Estado de este documento: referencia canónica activa para versionado y releases en la etapa `0.x`.

## Propósito

Esta política traduce el alcance `alpha` y la matriz de madurez a reglas prácticas de release.

No define una política `1.0`, no inventa automatización nueva y no reemplaza la documentación de alcance del SDK `alpha`.

## Estado actual de versionado

`mine-rs` debe leerse hoy como un SDK `alpha` en serie `0.x`.

Eso significa:

- todavía no existe promesa de estabilidad `1.0`;
- la API pública sigue consolidándose;
- pueden existir cambios de breaking change entre releases `0.x`, pero no deben comunicarse de forma implícita.

## Superficies con compatibilidad cuidada

La compatibilidad que hoy se cuida de forma explícita aplica solo a las superficies **recomendadas** de la etapa `alpha`:

- `mine-sdk` como entrada pública recomendada en Rust;
- `miners` como paquete público recomendado en Python;
- `miners.tools` como superficie recomendada de tools deterministas.

En estas superficies, la expectativa correcta para `0.x` es:

- preservar cuando sea razonable los imports, nombres y flujos recomendados ya documentados;
- evitar breaking changes innecesarios en el camino recomendado;
- si un release `0.x` rompe una de estas superficies, incluir notas de release y notas de migración explícitas.

Esto es una garantía de cuidado y comunicación, no una promesa de compatibilidad fuerte estilo `1.0`.

## Superficies no contractuales en `0.x`

Las superficies marcadas como **experimental** o **benchmark-side** no forman parte del contrato de compatibilidad del release.

Esto incluye, entre otras:

- `mine_sdk::experimental`;
- `miners.experimental`;
- harnesses, adaptadores y ejemplos benchmark-side de MineLib, Marvin, McLaughlin o investigación similar.

Estas áreas pueden cambiar, moverse, renombrarse o desaparecer entre releases `0.x` sin el mismo nivel de preservación exigido para `mine-sdk`, `miners` y `miners.tools`.

## Regla para breaking changes en superficies recomendadas

Si un release `0.x` introduce un breaking change sobre `mine-sdk`, `miners` o `miners.tools`, el cambio debe salir acompañado por:

- notas de release que nombren el quiebre de forma explícita;
- notas de migración cortas con el cambio esperado para usuarios del camino recomendado.

La regla práctica es simple:

```text
romper una superficie recomendada en `0.x` puede ser válido;
hacerlo sin notas explícitas no lo es.
```

## Gates mínimos de release existentes hoy

Hoy el mínimo aceptable antes de tratar un estado del repo como releaseable en `0.x` es la evidencia que ya existe:

1. **Rust CI** verde en el workflow actual del repo:
   - `cargo build --workspace`
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo nextest run --workspace`
2. **Flujo local Python soportado** validado para la superficie alpha:
   - `python -m maturin develop`
   - `python -m unittest discover -s tests -p "test_python_*.py"`

Estos gates son mínimos y actuales. No implican todavía una política completa de publishing, wheels, firmas, canales automáticos ni rediseño de CI.

## Lectura correcta de esta política

- `0.x` alpha: sí.
- Superficies recomendadas con compatibilidad cuidada: sí.
- Superficies experimentales y benchmark-side como no contractuales: sí.
- Promesa de estabilidad amplia `1.0`: no.

## Relación con otros documentos

- `docs/references/sdk-alpha-scope.md`: define el alcance y claims del SDK `alpha`.
- `docs/references/maturity-matrix.md`: clasifica superficies en estable, experimental y benchmark-side.
- `docs/references/python-sdk-design.md`: fija el flujo local soportado para validar la superficie Python actual.
