//! A product can answer to many barcodes: variations of the same goods, or a pack that counts as many.

use ravenrs_app::{FieldValue, Object, ObjectSpace, Query, SpaceError, Value};
use ravenrs_pos::conferir_codigo;
use rust_decimal::Decimal;

use crate::caixa::texto;
use crate::modelo::{CODIGO, PRODUTO};

/// What a code read at the counter leads to.
#[derive(Clone, Debug, PartialEq)]
pub struct Encontrado {
    /// The product.
    pub produto: Object,
    /// How many units it counts as.
    pub fator: Decimal,
}

/// The product of a code: its own barcode, its internal code or any of its other barcodes.
pub fn produto_por_codigo(space: &dyn ObjectSpace, codigo: &str) -> Option<Encontrado> {
    let produtos = space.query(PRODUTO, &Query::all()).ok()?;
    let proprio =
        produtos.iter().find(|produto| texto(produto, "gtin") == codigo || texto(produto, "codigo") == codigo);
    if let Some(produto) = proprio {
        return Some(Encontrado { produto: produto.clone(), fator: Decimal::ONE });
    }
    let codigos = space.query(CODIGO, &Query::all()).ok()?;
    let achado = codigos.iter().find(|linha| texto(linha, "codigo") == codigo)?;
    let dono = texto(achado, "produto");
    let produto = produtos.into_iter().find(|produto| texto(produto, "codigo") == dono)?;
    let fator = Decimal::from_value(achado.get("fator"));
    Some(Encontrado { produto, fator: if fator > Decimal::ZERO { fator } else { Decimal::ONE } })
}

/// Gives a product one more barcode, in the unit of work. Says so when the code cannot be used or
/// already belongs to another product. Adding one it already has changes nothing.
pub fn cadastrar_codigo(
    space: &mut dyn ObjectSpace,
    produto: &str,
    codigo: &str,
    descricao: &str,
    fator: Decimal,
) -> Result<(), SpaceError> {
    let codigo = conferir_codigo(codigo).map_err(SpaceError::Conflict)?;
    if fator <= Decimal::ZERO {
        return Err(SpaceError::Conflict("O código precisa valer ao menos uma unidade.".to_owned()));
    }
    if let Some(encontrado) = produto_por_codigo(&*space, &codigo) {
        let dono = texto(&encontrado.produto, "codigo");
        if dono == produto {
            return Ok(());
        }
        return Err(SpaceError::Conflict(format!("O código {codigo} já é do produto {dono}.")));
    }
    let mut linha = space.new_object(CODIGO)?;
    linha.set("produto", produto.to_owned());
    linha.set("codigo", codigo);
    linha.set("descricao", descricao.to_owned());
    linha.set("fator", Decimal::to_value(&fator));
    space.save(linha)
}

/// The other barcodes of a product.
pub fn codigos_do_produto(space: &dyn ObjectSpace, produto: &str) -> Vec<(String, Decimal, Value)> {
    space
        .query(CODIGO, &Query::all())
        .unwrap_or_default()
        .iter()
        .filter(|linha| texto(linha, "produto") == produto)
        .map(|linha| (texto(linha, "codigo"), Decimal::from_value(linha.get("fator")), linha.get("id")))
        .collect()
}
