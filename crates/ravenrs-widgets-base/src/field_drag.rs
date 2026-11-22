//! A field of data carried by the pointer, from where data is looked at (a `DataInspector`) to
//! where an expression is written (an `ExpressionEditor`, a field of a `ParameterForm`): what
//! people see and hear while carrying it, and the expression that reads it.

use crate::context::DragPayload;

/// The kind of a [`DragPayload`] carrying a [`FieldDrag`].
pub const FIELD_DRAG: &str = "field";

/// A field of data being carried: its path as people read it and the expression that reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FieldDrag {
    /// The path as people read it, as `cliente.nome` or `itens[0].preço`.
    pub path: String,
    /// The expression of the criteria language that reads it, as `[cliente.nome]`.
    pub expression: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FieldDrag {
    /// The field at `path`, read by `expression`.
    pub fn new(path: impl Into<String>, expression: impl Into<String>) -> Self {
        Self { path: path.into(), expression: expression.into() }
    }

    /// What a drag of it carries.
    pub fn payload(&self) -> DragPayload {
        DragPayload::new(FIELD_DRAG, &self.path, self.clone())
    }

    /// The field a drag carries, when it carries one.
    pub fn carried(payload: &DragPayload) -> Option<&FieldDrag> {
        if payload.kind() == FIELD_DRAG { payload.data::<FieldDrag>() } else { None }
    }
}

/// The expression of the criteria language that reads the field at these keys (a row of a
/// list is its number): `["cliente", "nome"]` reads `[cliente.nome]`.
pub fn field_expression(keys: &[String]) -> String {
    format!("[{}]", keys.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_travels_in_a_payload_and_comes_back_whole() {
        let field =
            FieldDrag::new("itens[0].preço", field_expression(&["itens".into(), "0".into(), "preço".into()]));
        assert_eq!(field.expression, "[itens.0.preço]");
        let payload = field.payload();
        assert_eq!(payload.label(), "itens[0].preço");
        assert_eq!(FieldDrag::carried(&payload), Some(&field));
        assert_eq!(FieldDrag::carried(&DragPayload::new("other", "x", 1_u8)), None);
    }
}
