//! Centro de Cursos Exemplo: matrícula fora do prazo autorizada pela coordenação, numa etapa só.
//!
//! Usa o mesmo pacote `ravenrs-app-requests` da associação, com outra etapa, outro papel e outra
//! regra: aqui um pedido sem polo não é decidido, e quem pede pode ser da própria coordenação.

use std::rc::Rc;

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Value};
use ravenrs_app_requests::{RequestsModule, Step};

/// Papel de quem autoriza.
pub const COORDENACAO: &str = "Coordenação";

/// O pacote com a etapa única do centro de cursos.
pub fn matriculas() -> Result<RequestsModule, String> {
    RequestsModule::new(vec![Step::new("coordenacao", "Autorização da coordenação", COORDENACAO)]).map(
        |pacote| {
            pacote.policy(Rc::new(|_, pedido, _| match pedido.get("unit") {
                Value::Text(polo) if !polo.trim().is_empty() => Ok(()),
                _ => Err("Diga o polo da matrícula antes de autorizar.".to_owned()),
            }))
        },
    )
}

/// Cursos, que só esta aplicação tem.
pub fn curso() -> EntityInfo {
    EntityInfo::new("Curso")
        .field(FieldInfo::new("codigo", FieldKind::Text).key())
        .field(FieldInfo::new("nome", FieldKind::Text).default_property())
        .field(FieldInfo::new("vagas", FieldKind::Integer))
}

/// Tudo o que o centro guarda: o que é dele e o que o pacote declara.
pub fn entidades() -> Result<Vec<EntityInfo>, String> {
    let mut todas = vec![curso()];
    todas.extend(matriculas()?.declared_entities());
    Ok(todas)
}
