//! Associação Exemplo: compras aprovadas pelo conselho fiscal e depois pela diretoria.
//!
//! Usa o pacote `ravenrs-app-requests` como ele é publicado: as etapas, os papéis e a regra de que
//! quem pede não decide são desta aplicação; o pacote não sabe nada de associações.

use ravenrs_app::{EntityInfo, FieldInfo, FieldKind};
use ravenrs_app_requests::{RequestsModule, Step, forbid_self_decision};

/// Papel de quem confere as compras.
pub const CONSELHO: &str = "Conselho fiscal";
/// Papel de quem aprova por último.
pub const DIRETORIA: &str = "Diretoria";

/// O pacote com as duas etapas da associação.
pub fn compras() -> Result<RequestsModule, String> {
    RequestsModule::new(vec![
        Step::new("conselho", "Conferência do conselho", CONSELHO),
        Step::new("diretoria", "Aprovação da diretoria", DIRETORIA),
    ])
    .map(|pacote| pacote.policy(forbid_self_decision()))
}

/// Fornecedores, que só esta aplicação tem.
pub fn fornecedor() -> EntityInfo {
    EntityInfo::new("Fornecedor")
        .field(FieldInfo::new("cnpj", FieldKind::Text).key())
        .field(FieldInfo::new("nome", FieldKind::Text).default_property())
}

/// Tudo o que a associação guarda: o que é dela, o que o pacote declara e os eventos a enviar.
pub fn entidades() -> Result<Vec<EntityInfo>, String> {
    let mut todas = vec![fornecedor()];
    todas.extend(compras()?.declared_entities());
    todas.push(ravenrs_app::outbox_entity());
    Ok(todas)
}
