//! The records of a point of sale.

use ravenrs_app::{EntityInfo, EnumOption, FieldInfo, FieldKind, Rule};
use ravenrs_app_nfe::NfeModule;

/// The entity of products.
pub const PRODUTO: &str = "PosProduto";
/// The entity of the settings of the checkout.
pub const CONFIGURACAO: &str = "PosConfiguracao";
/// The entity of customers.
pub const CLIENTE: &str = "PosCliente";
/// The entity of what suppliers call products.
pub const VINCULO: &str = "PosVinculoFornecedor";
/// The entity of what came in or went out of stock.
pub const MOVIMENTO: &str = "PosMovimento";
/// The entity of the other barcodes of products.
pub const CODIGO: &str = "PosCodigoBarras";
/// The entity of services, sold with an NFS-e.
pub const SERVICO: &str = "PosServico";

/// What is sold as a service, with an NFS-e of the national standard.
pub fn servico_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption);
    EntityInfo::new(SERVICO)
        .caption("Serviços")
        .navigation("Cadastros")
        .icon("wrench")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("descricao", "Descrição").default_property())
        .field(text("codigo", "Código interno"))
        .field(text("codigo_tributacao_nacional", "Código de tributação nacional (cTribNac)"))
        .field(FieldInfo::new("preco", FieldKind::Money).caption("Preço").default_value(0.0))
        .rule(Rule::required("descricao"))
        .rule(Rule::required("codigo"))
        .rule(Rule::unique("codigo"))
        .rule(
            Rule::matches("codigo_tributacao_nacional", "^[0-9]{6}$")
                .message("O código de tributação nacional tem 6 dígitos: item, subitem e desdobro."),
        )
}

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
        .field(FieldInfo::new("custo", FieldKind::Decimal(4)).caption("Custo médio").default_value(0.0))
        .field(FieldInfo::new("estoque", FieldKind::Decimal(3)).caption("Estoque").default_value(0.0))
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
        .field(text("driver_cupom", "Driver da impressora de cupom").default_value("elgin.i9.cupom"))
        .field(
            text("driver_etiqueta", "Driver da impressora de etiquetas")
                .default_value("elgin.l42-pro.etiqueta"),
        )
        .field(
            FieldInfo::new("impostos", FieldKind::Decimal(2))
                .caption("Impostos sobre a venda (%)")
                .default_value(6.0),
        )
        .field(
            FieldInfo::new("custos_fixos", FieldKind::Decimal(2))
                .caption("Custos fixos (%)")
                .default_value(15.0),
        )
        .field(
            FieldInfo::new("comissao", FieldKind::Decimal(2))
                .caption("Comissão e taxas (%)")
                .default_value(2.0),
        )
        .field(
            FieldInfo::new("margem", FieldKind::Decimal(2))
                .caption("Margem desejada (%)")
                .default_value(12.0),
        )
        .field(
            FieldInfo::new("final_de_preco", FieldKind::Integer)
                .caption("Centavos do preço de gôndola")
                .default_value(90.0),
        )
}

/// What a supplier calls a product, and how many of ours come in one of theirs.
pub fn vinculo_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption);
    EntityInfo::new(VINCULO)
        .caption("Produtos dos fornecedores")
        .navigation("Cadastros")
        .icon("link")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("fornecedor", "CNPJ do fornecedor").default_property())
        .field(text("codigo_fornecedor", "Código do fornecedor"))
        .field(text("produto", "Código interno do produto"))
        .field(
            FieldInfo::new("fator", FieldKind::Decimal(4))
                .caption("Unidades por embalagem")
                .default_value(1.0),
        )
}

/// Other barcodes a product answers to.
pub fn codigo_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption);
    EntityInfo::new(CODIGO)
        .caption("Códigos de barras")
        .navigation("Cadastros")
        .icon("scan-line")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("codigo", "Código").default_property())
        .field(text("produto", "Código interno do produto"))
        .field(text("descricao", "O que este código é"))
        .field(
            FieldInfo::new("fator", FieldKind::Decimal(4))
                .caption("Vale quantas unidades")
                .default_value(1.0),
        )
        .rule(Rule::required("codigo"))
        .rule(Rule::required("produto"))
        .rule(Rule::unique("codigo"))
}

/// What came in or went out of stock.
pub fn movimento_entity() -> EntityInfo {
    let text = |name: &str, caption: &str| FieldInfo::new(name, FieldKind::Text).caption(caption).read_only();
    EntityInfo::new(MOVIMENTO)
        .caption("Movimentos de estoque")
        .navigation("Cadastros")
        .icon("arrow-left-right")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(text("produto", "Produto").default_property())
        .field(text("origem", "Origem"))
        .field(text("documento", "Documento"))
        .field(FieldInfo::new("quantidade", FieldKind::Decimal(3)).caption("Quantidade").read_only())
        .field(FieldInfo::new("custo", FieldKind::Decimal(4)).caption("Custo").read_only())
}

/// Everything storage has to know for a point of sale.
pub fn entidades() -> Vec<EntityInfo> {
    let mut all = vec![
        produto_entity(),
        servico_entity(),
        cliente_entity(),
        configuracao_entity(),
        vinculo_entity(),
        codigo_entity(),
        movimento_entity(),
        ravenrs_app::sequence_entity(),
        ravenrs_app::draft_entity(),
        ravenrs_app_report::report_entity(),
    ];
    all.extend(NfeModule::entidades());
    all
}
