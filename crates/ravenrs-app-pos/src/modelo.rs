//! The records of a point of sale.

use ravenrs_app::{EntityInfo, EnumOption, FieldInfo, FieldKind, Rule};
use ravenrs_app_nfe::NfeModule;

/// The entity of products.
pub const PRODUTO: &str = "PosProduto";
/// The entity of the settings of the checkout.
pub const CONFIGURACAO: &str = "PosConfiguracao";
/// The entity of customers.
pub const CLIENTE: &str = "PosCliente";

/// What is sold.
pub fn produto_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption);
    EntityInfo::new(PRODUTO)
        .caption("Produtos")
        .navigation("Cadastros")
        .icon("package")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("descricao", "Descrição").default_property())
        .field(text("codigo", "Código interno"))
        .field(text("gtin", "Código de barras (GTIN)").editor("Gtin"))
        .field(FieldInfo::new("preco", FieldKind::Money).caption("Preço de venda").default_value(0.0))
        .field(text("unidade", "Unidade").default_value("UN"))
        .field(text("ncm", "NCM"))
        .field(text("cfop", "CFOP").default_value("5102"))
        .field(text("cest", "CEST"))
        .field(
            FieldInfo::new(
                "csosn",
                FieldKind::Enum(vec![
                    EnumOption::new("102", "102, sem crédito"),
                    EnumOption::new("500", "500, ICMS por substituição"),
                ]),
            )
            .caption("CSOSN")
            .default_value("102"),
        )
        .rule(Rule::required("descricao"))
        .rule(Rule::required("codigo"))
        .rule(Rule::unique("codigo"))
        .rule(Rule::matches("ncm", "^[0-9]{8}$").message("O NCM tem 8 dígitos."))
        .rule(Rule::matches("cfop", "^[0-9]{4}$").message("O CFOP tem 4 dígitos."))
}

/// Who buys with a note of their own: a company, usually.
pub fn cliente_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption);
    EntityInfo::new(CLIENTE)
        .caption("Clientes")
        .navigation("Cadastros")
        .icon("users")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("nome", "Nome ou razão social").default_property())
        .field(text("documento", "CPF ou CNPJ"))
        .field(text("logradouro", "Logradouro"))
        .field(text("numero", "Número"))
        .field(text("bairro", "Bairro"))
        .field(text("codigo_municipio", "Código IBGE do município"))
        .field(text("municipio", "Município"))
        .field(text("uf", "UF"))
        .field(text("cep", "CEP"))
        .rule(Rule::required("nome"))
        .rule(Rule::required("documento"))
        .rule(Rule::unique("documento"))
}

/// The settings of the checkout: one record.
pub fn configuracao_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption);
    EntityInfo::new(CONFIGURACAO)
        .caption("Configuração do caixa")
        .navigation("Cadastros")
        .icon("settings")
        .singleton()
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("caixa", "Caixa").default_value("01").default_property())
        .field(text("operador", "Operador"))
        .field(text("pin", "PIN para desbloquear").default_value("1234"))
        .field(text("impressora_cupom", "Impressora de cupom").default_value(""))
        .field(text("impressora_etiqueta", "Impressora de etiquetas").default_value(""))
}

/// Everything storage has to know for a point of sale.
pub fn entidades() -> Vec<EntityInfo> {
    let mut all = vec![
        produto_entity(),
        cliente_entity(),
        configuracao_entity(),
        ravenrs_app::sequence_entity(),
        ravenrs_app::draft_entity(),
        ravenrs_app_report::report_entity(),
    ];
    all.extend(NfeModule::entidades());
    all
}
