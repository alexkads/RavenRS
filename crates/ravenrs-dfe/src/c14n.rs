//! Inclusive Canonical XML 1.0 without comments (REC-xml-c14n-20010315).
//!
//! The rules implemented, in the order of the recommendation: line ends and references come
//! resolved from [`crate::xml`], empty elements are expanded, namespace declarations are
//! written first and sorted by prefix, attributes are sorted by namespace URI and local name,
//! declarations the output parent already made are dropped, and an apex below the root
//! inherits the namespaces and `xml:*` attributes of its ancestors.

use std::collections::BTreeMap;

use crate::erro::DfeError;
use crate::xml::{Document, Element, Node};

const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const DSIG_NS: &str = "http://www.w3.org/2000/09/xmldsig#";

/// The namespaces in scope: prefix to URI, the default namespace under the empty prefix.
type Scope = BTreeMap<String, String>;

/// Canonicalizes a whole document, with the processing instructions around its root.
pub fn canonicalize_document(document: &Document) -> Result<String, DfeError> {
    let mut out = String::new();
    for node in &document.prolog {
        write_pi(&mut out, node);
        out.push('\n');
    }
    let options = Options { omit_signature: false };
    write_element(&mut out, &document.root, &Scope::new(), &Scope::new(), &[], options)?;
    for node in &document.epilog {
        out.push('\n');
        write_pi(&mut out, node);
    }
    Ok(out)
}

/// Canonicalizes the element whose `Id` is `id`, taken from the tree under `root`.
///
/// `omit_signature` removes `Signature` elements of XMLDSig, which is the
/// `enveloped-signature` transform.
pub fn canonicalize_by_id(root: &Element, id: &str, omit_signature: bool) -> Result<String, DfeError> {
    canonicalize_where(root, &|element| element.attr("Id") == Some(id), omit_signature)
        .ok_or_else(|| DfeError::Xml(format!("elemento com Id {id} não encontrado")))?
}

/// Canonicalizes the first element with this local name, taken from the tree under `root`.
pub fn canonicalize_by_name(root: &Element, local: &str) -> Result<String, DfeError> {
    canonicalize_where(root, &|element| element.local_name() == local, false)
        .ok_or_else(|| DfeError::Xml(format!("elemento {local} não encontrado")))?
}

fn canonicalize_where(
    root: &Element,
    matches: &dyn Fn(&Element) -> bool,
    omit_signature: bool,
) -> Option<Result<String, DfeError>> {
    let mut path = Vec::new();
    if !find_path(root, matches, &mut path) {
        return None;
    }
    let (apex, ancestors) = path.split_last()?;
    let mut scope = Scope::new();
    let mut inherited: BTreeMap<&str, &str> = BTreeMap::new();
    for ancestor in ancestors {
        if let Err(erro) = extend_scope(&mut scope, ancestor) {
            return Some(Err(erro));
        }
        for (key, value) in &ancestor.attrs {
            if key.starts_with("xml:") {
                inherited.insert(key, value);
            }
        }
    }
    let inherited: Vec<(&str, &str)> =
        inherited.into_iter().filter(|(key, _)| apex.attr(key).is_none()).collect();
    let mut out = String::new();
    let options = Options { omit_signature };
    Some(write_element(&mut out, apex, &scope, &Scope::new(), &inherited, options).map(|()| out))
}

fn find_path<'a>(
    element: &'a Element,
    matches: &dyn Fn(&Element) -> bool,
    path: &mut Vec<&'a Element>,
) -> bool {
    path.push(element);
    if matches(element) {
        return true;
    }
    for child in element.elements() {
        if find_path(child, matches, path) {
            return true;
        }
    }
    path.pop();
    false
}

#[derive(Clone, Copy)]
struct Options {
    omit_signature: bool,
}

fn extend_scope(scope: &mut Scope, element: &Element) -> Result<(), DfeError> {
    for (key, value) in &element.attrs {
        if key == "xmlns" {
            scope.insert(String::new(), value.clone());
        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
            if value.is_empty() {
                return Err(DfeError::Xml(format!("prefixo {prefix} sem namespace")));
            }
            scope.insert(prefix.to_string(), value.clone());
        }
    }
    Ok(())
}

fn split_prefix(name: &str) -> (&str, &str) {
    name.split_once(':').unwrap_or(("", name))
}

fn is_signature(element: &Element, scope: &Scope) -> bool {
    let (prefix, local) = split_prefix(&element.name);
    local == "Signature" && scope.get(prefix).is_some_and(|uri| uri == DSIG_NS)
}

fn write_element(
    out: &mut String,
    element: &Element,
    parent_scope: &Scope,
    rendered: &Scope,
    inherited_xml: &[(&str, &str)],
    options: Options,
) -> Result<(), DfeError> {
    let mut scope = parent_scope.clone();
    extend_scope(&mut scope, element)?;

    out.push('<');
    out.push_str(&element.name);

    // Namespace declarations first, by prefix; the BTreeMap puts the default one ahead.
    let mut now_rendered = rendered.clone();
    for (prefix, uri) in &scope {
        if prefix == "xml" && uri == XML_NS {
            continue;
        }
        let in_parent = rendered.get(prefix).map_or("", String::as_str);
        if in_parent == uri {
            continue;
        }
        if prefix.is_empty() {
            out.push_str(" xmlns=\"");
        } else {
            out.push_str(" xmlns:");
            out.push_str(prefix);
            out.push_str("=\"");
        }
        write_attr_value(out, uri);
        out.push('"');
        now_rendered.insert(prefix.clone(), uri.clone());
    }

    // Then the attributes, by namespace URI and local name.
    let mut attributes: Vec<(String, &str, &str, &str)> = Vec::new();
    let plain = element
        .attrs
        .iter()
        .filter(|(key, _)| key != "xmlns" && !key.starts_with("xmlns:"))
        .map(|(key, value)| (key.as_str(), value.as_str()));
    for (key, value) in plain.chain(inherited_xml.iter().copied()) {
        let (prefix, local) = split_prefix(key);
        let uri = match prefix {
            "" => String::new(),
            "xml" => XML_NS.to_string(),
            other => scope
                .get(other)
                .cloned()
                .ok_or_else(|| DfeError::Xml(format!("prefixo {other} não declarado")))?,
        };
        attributes.push((uri, local, key, value));
    }
    attributes.sort_by(|a, b| (a.0.as_str(), a.1).cmp(&(b.0.as_str(), b.1)));
    for (_, _, key, value) in &attributes {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        write_attr_value(out, value);
        out.push('"');
    }
    out.push('>');

    for node in &element.children {
        match node {
            Node::Element(child) => {
                if options.omit_signature && {
                    let mut child_scope = scope.clone();
                    extend_scope(&mut child_scope, child)?;
                    is_signature(child, &child_scope)
                } {
                    continue;
                }
                write_element(out, child, &scope, &now_rendered, &[], options)?;
            }
            Node::Text(text) => write_text(out, text),
            pi @ Node::Pi { .. } => write_pi(out, pi),
        }
    }

    out.push_str("</");
    out.push_str(&element.name);
    out.push('>');
    Ok(())
}

fn write_pi(out: &mut String, node: &Node) {
    if let Node::Pi { target, data } = node {
        out.push_str("<?");
        out.push_str(target);
        if !data.is_empty() {
            out.push(' ');
            out.push_str(data);
        }
        out.push_str("?>");
    }
}

fn write_text(out: &mut String, text: &str) {
    for caractere in text.chars() {
        match caractere {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
            other => out.push(other),
        }
    }
}

fn write_attr_value(out: &mut String, value: &str) {
    for caractere in value.chars() {
        match caractere {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::{parse, parse_document};

    fn canon(xml: &str) -> String {
        canonicalize_document(&parse_document(xml).expect("xml")).expect("c14n")
    }

    #[test]
    fn expands_empty_elements_and_sorts_attributes() {
        assert_eq!(canon("<a z=\"1\" b=\"2\"/>"), "<a b=\"2\" z=\"1\"></a>");
    }

    #[test]
    fn namespaces_come_first_and_attributes_sort_by_uri() {
        let xml = "<a b:x=\"1\" y=\"2\" a:x=\"3\" xmlns:b=\"urn:1\" xmlns:a=\"urn:2\" xmlns=\"urn:0\"/>";
        assert_eq!(
            canon(xml),
            "<a xmlns=\"urn:0\" xmlns:a=\"urn:2\" xmlns:b=\"urn:1\" y=\"2\" b:x=\"1\" a:x=\"3\"></a>"
        );
    }

    #[test]
    fn repeated_declarations_are_dropped() {
        let xml = "<a xmlns=\"u\"><b xmlns=\"u\"/><c xmlns=\"\"/></a>";
        assert_eq!(canon(xml), "<a xmlns=\"u\"><b></b><c xmlns=\"\"></c></a>");
    }

    #[test]
    fn text_and_attributes_are_escaped() {
        let xml = "<a t=\"&quot;&#xA;&lt;&amp;'\">&lt;&gt;&amp;&#xD;\"'</a>";
        assert_eq!(canon(xml), "<a t=\"&quot;&#xA;&lt;&amp;'\">&lt;&gt;&amp;&#xD;\"'</a>");
    }

    #[test]
    fn comments_and_declaration_go_away() {
        let xml = "<?xml version=\"1.0\"?><a><!-- x -->b</a><!-- y -->";
        assert_eq!(canon(xml), "<a>b</a>");
    }

    #[test]
    fn processing_instructions_keep_their_place() {
        let xml = "<?p  um?><a><?q?></a><?r dois?>";
        assert_eq!(canon(xml), "<?p um?>\n<a><?q?></a>\n<?r dois?>");
    }

    #[test]
    fn apex_inherits_namespaces_and_xml_attributes() {
        let root =
            parse("<s:e xmlns:s=\"urn:s\" xml:lang=\"pt\"><b xmlns=\"urn:b\" Id=\"x\"/></s:e>").expect("xml");
        assert_eq!(
            canonicalize_by_id(&root, "x", false).expect("c14n"),
            "<b xmlns=\"urn:b\" xmlns:s=\"urn:s\" Id=\"x\" xml:lang=\"pt\"></b>"
        );
    }

    #[test]
    fn enveloped_transform_removes_only_the_dsig_signature() {
        let xml = "<a Id=\"i\" xmlns=\"u\"><Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\"><x/></Signature><Signature/></a>";
        let root = parse(xml).expect("xml");
        assert_eq!(
            canonicalize_by_id(&root, "i", true).expect("c14n"),
            "<a xmlns=\"u\" Id=\"i\"><Signature></Signature></a>"
        );
    }

    #[test]
    fn undeclared_prefix_in_attribute_is_an_error() {
        let document = parse_document("<a p:x=\"1\"/>").expect("xml");
        assert!(canonicalize_document(&document).is_err());
    }
}
