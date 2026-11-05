//! A small XML tree over `quick-xml`, keeping what canonicalization needs.
//!
//! Document type declarations are refused, so no entity can be defined and no external
//! resource can be reached. Names stay as written, prefix included; only C14N resolves them.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::error::NfeError;

/// The deepest nesting accepted; no document of this layout comes close.
const PROFUNDIDADE_MAXIMA: usize = 128;

/// One node of an element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// A child element.
    Element(Element),
    /// Character data, with references already resolved.
    Text(String),
    /// A processing instruction.
    Pi {
        /// Its target.
        target: String,
        /// Its data, without the spaces after the target.
        data: String,
    },
}

/// An element with its attributes as written, namespace declarations included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    /// The name, with its prefix if it has one.
    pub name: String,
    /// The attributes in document order, with their values resolved.
    pub attrs: Vec<(String, String)>,
    /// The children in document order.
    pub children: Vec<Node>,
}

/// A whole document: what comes before the root, the root and what comes after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    /// Processing instructions before the root.
    pub prolog: Vec<Node>,
    /// The root element.
    pub root: Element,
    /// Processing instructions after the root.
    pub epilog: Vec<Node>,
}

impl Element {
    /// The name without its prefix.
    #[must_use]
    pub fn local_name(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    /// The value of an attribute, by the name as written.
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    /// The direct child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            _ => None,
        })
    }

    /// The first direct child element with this local name.
    #[must_use]
    pub fn child(&self, local: &str) -> Option<&Element> {
        self.elements().find(|element| element.local_name() == local)
    }

    /// The first descendant with this local name, this element included, depth first.
    #[must_use]
    pub fn find(&self, local: &str) -> Option<&Element> {
        if self.local_name() == local {
            return Some(self);
        }
        self.elements().find_map(|element| element.find(local))
    }

    /// All the text directly inside this element.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = String::new();
        for node in &self.children {
            if let Node::Text(part) = node {
                text.push_str(part);
            }
        }
        text
    }

    /// The text of the first direct child with this local name.
    #[must_use]
    pub fn child_text(&self, local: &str) -> Option<String> {
        self.child(local).map(Element::text)
    }

    /// The text of the first descendant with this local name.
    #[must_use]
    pub fn find_text(&self, local: &str) -> Option<String> {
        self.find(local).map(Element::text)
    }
}

/// Escapes text for an element's content in an XML this crate writes.
#[must_use]
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for caractere in text.chars() {
        match caractere {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

/// Escapes text for an attribute value in an XML this crate writes.
#[must_use]
pub fn escape_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for caractere in text.chars() {
        match caractere {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

/// Writes an element back as text, without any formatting of its own.
#[must_use]
pub fn to_string(element: &Element) -> String {
    let mut out = String::new();
    write_element(&mut out, element);
    out
}

fn write_element(out: &mut String, element: &Element) {
    out.push('<');
    out.push_str(&element.name);
    for (key, value) in &element.attrs {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(&escape_attr(value));
        out.push('"');
    }
    if element.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for node in &element.children {
        match node {
            Node::Element(child) => write_element(out, child),
            Node::Text(text) => out.push_str(&escape_text(text)),
            Node::Pi { target, data } => {
                out.push_str("<?");
                out.push_str(target);
                if !data.is_empty() {
                    out.push(' ');
                    out.push_str(data);
                }
                out.push_str("?>");
            }
        }
    }
    out.push_str("</");
    out.push_str(&element.name);
    out.push('>');
}

/// Reads a whole document.
pub fn parse_document(xml: &str) -> Result<Document, NfeError> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(false);
    config.check_end_names = true;
    config.expand_empty_elements = false;

    let mut stack: Vec<Element> = Vec::new();
    let mut prolog = Vec::new();
    let mut epilog = Vec::new();
    let mut root = None;

    loop {
        let event = reader.read_event().map_err(|erro| xml_error(&erro.to_string()))?;
        match event {
            Event::Decl(decl) => {
                if let Some(Ok(encoding)) = decl.encoding()
                    && !encoding.eq_ignore_ascii_case(b"utf-8")
                {
                    return Err(xml_error("só UTF-8 é aceito"));
                }
            }
            Event::DocType(_) => return Err(xml_error("declaração de tipo de documento recusada")),
            Event::Start(start) => {
                if stack.len() >= PROFUNDIDADE_MAXIMA {
                    return Err(xml_error("profundidade máxima excedida"));
                }
                stack.push(start_element(&start)?);
            }
            Event::Empty(start) => {
                let element = start_element(&start)?;
                attach(element, &mut stack, &mut root)?;
            }
            Event::End(_) => {
                let element = stack.pop().ok_or_else(|| xml_error("fechamento sem abertura"))?;
                attach(element, &mut stack, &mut root)?;
            }
            Event::Text(text) => {
                let decoded = text.decode().map_err(|erro| xml_error(&erro.to_string()))?;
                push_text(&mut stack, &normalize_lines(&decoded), root.is_some())?;
            }
            Event::CData(data) => {
                let decoded = data.decode().map_err(|erro| xml_error(&erro.to_string()))?;
                push_text(&mut stack, &normalize_lines(&decoded), root.is_some())?;
            }
            Event::GeneralRef(reference) => {
                let resolved = if reference.is_char_ref() {
                    let caractere = reference
                        .resolve_char_ref()
                        .map_err(|erro| xml_error(&erro.to_string()))?
                        .ok_or_else(|| xml_error("referência de caractere inválida"))?;
                    caractere.to_string()
                } else {
                    let name = reference.decode().map_err(|erro| xml_error(&erro.to_string()))?;
                    predefined(&name)?.to_string()
                };
                push_nonblank(&mut stack, &resolved)?;
            }
            Event::PI(pi) => {
                let target = String::from_utf8_lossy(pi.target()).into_owned();
                let data = String::from_utf8_lossy(pi.content()).trim_start().to_string();
                let node = Node::Pi { target, data };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else if root.is_some() {
                    epilog.push(node);
                } else {
                    prolog.push(node);
                }
            }
            Event::Comment(_) => {}
            Event::Eof => break,
        }
    }
    if !stack.is_empty() {
        return Err(xml_error("elemento sem fechamento"));
    }
    let root = root.ok_or_else(|| xml_error("documento sem elemento raiz"))?;
    Ok(Document { prolog, root, epilog })
}

/// Reads a document and keeps only its root element.
pub fn parse(xml: &str) -> Result<Element, NfeError> {
    parse_document(xml).map(|document| document.root)
}

fn xml_error(motivo: &str) -> NfeError {
    NfeError::Xml(motivo.to_string())
}

fn attach(element: Element, stack: &mut [Element], root: &mut Option<Element>) -> Result<(), NfeError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(Node::Element(element));
        Ok(())
    } else if root.is_none() {
        *root = Some(element);
        Ok(())
    } else {
        Err(xml_error("mais de um elemento raiz"))
    }
}

/// Adds text from the document; outside the root it may only be blank.
fn push_text(stack: &mut [Element], text: &str, _after_root: bool) -> Result<(), NfeError> {
    if stack.is_empty() {
        if text.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
            return Ok(());
        }
        return Err(xml_error("texto fora do elemento raiz"));
    }
    push_nonblank(stack, text)
}

fn push_nonblank(stack: &mut [Element], text: &str) -> Result<(), NfeError> {
    let Some(parent) = stack.last_mut() else {
        return Err(xml_error("referência fora do elemento raiz"));
    };
    if let Some(Node::Text(last)) = parent.children.last_mut() {
        last.push_str(text);
    } else if !text.is_empty() {
        parent.children.push(Node::Text(text.to_string()));
    }
    Ok(())
}

fn start_element(start: &BytesStart<'_>) -> Result<Element, NfeError> {
    let name =
        std::str::from_utf8(start.name().as_ref()).map_err(|erro| xml_error(&erro.to_string()))?.to_string();
    let mut attrs = Vec::new();
    for attribute in start.attributes().with_checks(true) {
        let attribute = attribute.map_err(|erro| xml_error(&erro.to_string()))?;
        let key = std::str::from_utf8(attribute.key.as_ref())
            .map_err(|erro| xml_error(&erro.to_string()))?
            .to_string();
        let raw = std::str::from_utf8(&attribute.value).map_err(|erro| xml_error(&erro.to_string()))?;
        attrs.push((key, attr_value(raw)?));
    }
    Ok(Element { name, attrs, children: Vec::new() })
}

/// Normalizes literal line ends to `\n`, the first step of every XML parser.
fn normalize_lines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Resolves an attribute value: literal white space becomes a space, references do not.
fn attr_value(raw: &str) -> Result<String, NfeError> {
    let normalized = normalize_lines(raw);
    let mut out = String::with_capacity(normalized.len());
    let mut rest = normalized.as_str();
    while let Some(position) = rest.find(['&', '\t', '\n', '<']) {
        out.push_str(&rest[..position]);
        let tail = &rest[position..];
        if tail.starts_with('<') {
            return Err(xml_error("'<' em valor de atributo"));
        }
        if !tail.starts_with('&') {
            out.push(' ');
            rest = &tail[1..];
            continue;
        }
        let end = tail.find(';').ok_or_else(|| xml_error("referência sem fim em atributo"))?;
        let name = &tail[1..end];
        if let Some(code) = name.strip_prefix('#') {
            let value = match code.strip_prefix('x') {
                Some(hex) => u32::from_str_radix(hex, 16),
                None => code.parse(),
            }
            .map_err(|_| xml_error("referência de caractere inválida"))?;
            let caractere = char::from_u32(value).ok_or_else(|| xml_error("caractere inválido"))?;
            out.push(caractere);
        } else {
            out.push(predefined(name)?);
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn predefined(name: &str) -> Result<char, NfeError> {
    match name {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "quot" => Ok('"'),
        "apos" => Ok('\''),
        _ => Err(xml_error("entidade desconhecida")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nested_elements_and_text() {
        let root = parse("<a x=\"1\"><b>texto &amp; mais</b><c/></a>").expect("xml");
        assert_eq!(root.attr("x"), Some("1"));
        assert_eq!(root.child_text("b").as_deref(), Some("texto & mais"));
        assert!(root.child("c").is_some());
    }

    #[test]
    fn refuses_a_doctype() {
        let xml = "<!DOCTYPE a [<!ENTITY x \"y\">]><a>&x;</a>";
        assert!(parse(xml).is_err());
    }

    #[test]
    fn refuses_an_unknown_entity() {
        assert!(parse("<a>&x;</a>").is_err());
    }

    #[test]
    fn refuses_mismatched_tags() {
        assert!(parse("<a><b></a></b>").is_err());
    }

    #[test]
    fn refuses_two_roots_and_text_outside() {
        assert!(parse("<a/><b/>").is_err());
        assert!(parse("x<a/>").is_err());
    }

    #[test]
    fn local_name_drops_the_prefix() {
        let root = parse("<s:a xmlns:s=\"u\"><s:b>1</s:b></s:a>").expect("xml");
        assert_eq!(root.local_name(), "a");
        assert_eq!(root.find_text("b").as_deref(), Some("1"));
    }
}
