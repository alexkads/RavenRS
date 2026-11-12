//! Masks personal data in whatever gets written down or exported.
//!
//! The issuer's CNPJ stays, because it is part of the access key and identifies the case. The
//! recipient, names, addresses, free text and the certificate do not.

use crate::xml::{self, Element, Node};

/// What goes in place of a certificate.
const SEM_CERTIFICADO: &str = "[certificado removido]";

/// Tags whose content is personal wherever they are, when an XML cannot be read as a tree.
const TAGS_SENSIVEIS: [&str; 15] = [
    "CPF",
    "CNPJ",
    "idEstrangeiro",
    "xNome",
    "xFant",
    "xLgr",
    "nro",
    "xCpl",
    "xBairro",
    "fone",
    "email",
    "IE",
    "infCpl",
    "xContato",
    "X509Certificate",
];

/// Masks a value and keeps enough to recognize it: `111******35`, or `***` when it is short.
#[must_use]
pub fn mascarar_valor(valor: &str) -> String {
    let caracteres: Vec<char> = valor.chars().collect();
    if caracteres.len() < 8 {
        return "***".to_string();
    }
    let inicio: String = caracteres[..3].iter().collect();
    let fim: String = caracteres[caracteres.len() - 2..].iter().collect();
    format!("{inicio}{}{fim}", "*".repeat(caracteres.len() - 5))
}

/// Whether the content of a tag under these parents is personal.
fn sensivel(pais: &[&str], tag: &str) -> bool {
    let dentro = |nome: &str| pais.contains(&nome);
    if tag == "X509Certificate" {
        return true;
    }
    if dentro("dest") || dentro("retirada") || dentro("entrega") || dentro("autXML") || dentro("transporta") {
        return true;
    }
    if dentro("emit") {
        return matches!(tag, "CPF" | "xNome" | "xFant" | "xLgr" | "nro" | "xCpl" | "xBairro" | "fone");
    }
    if dentro("infRespTec") {
        return matches!(tag, "xContato" | "email" | "fone");
    }
    matches!(tag, "infCpl" | "email")
}

/// Masks a value found at this path, if the path is personal. Used for each diagnosis.
#[must_use]
pub fn mascarar_em_caminho(caminho: &str, valor: &str) -> String {
    let partes: Vec<&str> = caminho
        .split('/')
        .filter(|parte| !parte.is_empty())
        .map(|parte| parte.split('[').next().unwrap_or(parte))
        .collect();
    let Some((tag, pais)) = partes.split_last() else {
        return valor.to_string();
    };
    if sensivel(pais, tag) { mascarar_valor(valor) } else { valor.to_string() }
}

fn mascarar_arvore(element: &mut Element, pais: &mut Vec<String>) {
    let nome = element.local_name().to_string();
    let referencias: Vec<&str> = pais.iter().map(String::as_str).collect();
    let trocar = sensivel(&referencias, &nome);
    let certificado = nome == "X509Certificate";
    pais.push(nome);
    for node in &mut element.children {
        match node {
            Node::Element(filho) => mascarar_arvore(filho, pais),
            Node::Text(texto) if trocar => {
                *texto = if certificado { SEM_CERTIFICADO.to_string() } else { mascarar_valor(texto) };
            }
            _ => {}
        }
    }
    pais.pop();
}

/// Masks text that could not be read as XML: known tags by name, then long runs of digits.
fn mascarar_texto_bruto(texto: &str) -> String {
    let mut saida = texto.to_string();
    for tag in TAGS_SENSIVEIS {
        let abre = format!("<{tag}>");
        let fecha = format!("</{tag}>");
        let mut inicio_busca = 0;
        while let Some(posicao) = saida[inicio_busca..].find(&abre) {
            let inicio = inicio_busca + posicao + abre.len();
            let fim = saida[inicio..].find(&fecha).map_or(saida.len(), |f| inicio + f);
            let troca = if tag == "X509Certificate" {
                SEM_CERTIFICADO.to_string()
            } else {
                mascarar_valor(&saida[inicio..fim])
            };
            saida.replace_range(inicio..fim, &troca);
            inicio_busca = inicio + troca.len();
        }
    }
    mascarar_digitos_soltos(&saida)
}

/// Masks runs of exactly 11 or 14 digits, the size of a CPF or a CNPJ.
fn mascarar_digitos_soltos(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut corrente = String::new();
    let fechar = |corrente: &mut String, saida: &mut String| {
        if matches!(corrente.len(), 11 | 14) {
            saida.push_str(&mascarar_valor(corrente));
        } else {
            saida.push_str(corrente);
        }
        corrente.clear();
    };
    for caractere in texto.chars() {
        if caractere.is_ascii_digit() {
            corrente.push(caractere);
        } else {
            fechar(&mut corrente, &mut saida);
            saida.push(caractere);
        }
    }
    fechar(&mut corrente, &mut saida);
    saida
}

/// Masks a report in plain text, where a tax id could only appear as a loose run of digits.
#[must_use]
pub fn mascarar_texto_de_relatorio(texto: &str) -> String {
    mascarar_digitos_soltos(texto)
}

/// Masks an XML text. What cannot be read as a tree still goes through the known tags.
#[must_use]
pub fn mascarar_xml(texto: &str) -> String {
    match xml::parse(texto) {
        Ok(mut raiz) => {
            mascarar_arvore(&mut raiz, &mut Vec::new());
            xml::to_string(&raiz)
        }
        Err(_) => mascarar_texto_bruto(texto),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_keeps_only_the_ends() {
        assert_eq!(mascarar_valor("11144477735"), "111******35");
        assert_eq!(mascarar_valor("ANA"), "***");
    }

    #[test]
    fn recipient_goes_and_issuer_cnpj_stays() {
        let xml = "<NFe><emit><CNPJ>11222333000181</CNPJ><xNome>MERCADO ABC</xNome></emit><dest><CPF>11144477735</CPF><xNome>ANA SILVA</xNome><enderDest><xLgr>RUA A DA SILVA</xLgr></enderDest></dest><X509Certificate>MIIB</X509Certificate></NFe>";
        let saida = mascarar_xml(xml);
        assert!(saida.contains("<CNPJ>11222333000181</CNPJ>"));
        for segredo in ["11144477735", "ANA SILVA", "MERCADO ABC", "RUA A DA SILVA", "MIIB"] {
            assert!(!saida.contains(segredo), "{segredo}");
        }
        assert!(saida.contains(SEM_CERTIFICADO));
    }

    #[test]
    fn broken_xml_is_still_masked() {
        let xml = "<dest><CPF>11144477735</CPF><xNome>ANA SILVA";
        let saida = mascarar_xml(xml);
        assert!(!saida.contains("11144477735"));
        assert!(!saida.contains("ANA SILVA"));
        assert_eq!(mascarar_xml("cpf 11144477735 aqui"), "cpf 111******35 aqui");
    }

    #[test]
    fn access_key_is_not_a_loose_tax_id() {
        let chave = "35261111222333000181650010000000011123456786";
        assert_eq!(mascarar_digitos_soltos(chave), chave);
    }

    #[test]
    fn path_rule_matches_the_tree_rule() {
        assert_eq!(mascarar_em_caminho("/NFe/infNFe/dest/CPF", "11144477735"), "111******35");
        assert_eq!(mascarar_em_caminho("/NFe/infNFe/emit/CNPJ", "11222333000181"), "11222333000181");
        assert_eq!(mascarar_em_caminho("/NFe/infNFe/det[1]/prod/vProd", "1.00"), "1.00");
    }
}
