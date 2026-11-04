//! Selector engine: CSS selectors, a practical XPath subset, and HTML/text
//! helpers.
//!
//! CSS selectors use the `scraper` crate (the same parser used for extraction).
//! XPath is implemented over the same parsed DOM so both selector styles see
//! exactly the same document. The supported XPath subset is deliberate and
//! documented in [`SUPPORTED_XPATH`]: absolute and relative location paths,
//! `*`, element names, `@attribute`, `text()`, positional predicates,
//! `last()`, attribute predicates and `contains()`.

use scraper::{ElementRef, Html, Selector};

use crate::error::{AicrawlError, Result};

/// Human-readable description of the supported XPath subset.
pub const SUPPORTED_XPATH: &str = "Supported XPath: /a/b, //a/b, *, element names, \
@attribute, text(), [n], [last()], [@attr], [@attr='value'], \
[contains(@attr,'value')], [contains(text(),'value')], [text()='value']. \
Axes, functions other than contains() and unions are not supported.";

/// A selected node returned by the selector engine.
#[derive(Debug, Clone)]
pub enum Node<'a> {
    /// An element in the parsed document.
    Element(ElementRef<'a>),
    /// Text content produced by a `text()` terminal.
    Text(String),
    /// Attribute value produced by an `@attribute` terminal.
    Attribute(String),
}

impl<'a> Node<'a> {
    /// Returns the element when this node is one.
    pub fn element(&self) -> Option<ElementRef<'a>> {
        match self {
            Self::Element(element) => Some(*element),
            _ => None,
        }
    }

    /// Returns the string value of a text or attribute node.
    pub fn literal(&self) -> Option<&str> {
        match self {
            Self::Text(text) | Self::Attribute(text) => Some(text),
            Self::Element(_) => None,
        }
    }
}

/// Validates a CSS selector without a document.
pub fn validate_css(selector: &str) -> Result<()> {
    if selector.trim().is_empty() {
        return Err(AicrawlError::InvalidInput("selector is empty".into()));
    }
    Selector::parse(selector)
        .map(|_| ())
        .map_err(|e| AicrawlError::InvalidInput(format!("{e:?}")))
}

/// Validates an XPath expression without a document.
pub fn validate_xpath(expression: &str) -> Result<()> {
    Expr::parse(expression).map(|_| ())
}

/// Selects nodes with CSS.
pub fn select_css<'a>(scope: &ElementRef<'a>, selector: &str) -> Result<Vec<Node<'a>>> {
    let parsed =
        Selector::parse(selector).map_err(|e| AicrawlError::InvalidInput(format!("{e:?}")))?;
    Ok(scope.select(&parsed).map(Node::Element).collect())
}

/// Selects nodes with the supported XPath subset.
pub fn select_xpath<'a>(scope: &ElementRef<'a>, expression: &str) -> Result<Vec<Node<'a>>> {
    let parsed = Expr::parse(expression)?;
    Ok(parsed.run(scope))
}

/// Selects nodes using whichever locator is present.
pub fn select<'a>(
    scope: &ElementRef<'a>,
    css: Option<&str>,
    xpath: Option<&str>,
) -> Result<Vec<Node<'a>>> {
    match (css, xpath) {
        (_, Some(expression)) => select_xpath(scope, expression),
        (Some(selector), None) => select_css(scope, selector),
        (None, None) => Ok(vec![Node::Element(*scope)]),
    }
}

/// Parses an HTML document.
pub fn parse_html_document(body: &str) -> Html {
    Html::parse_document(body)
}

/// Raw concatenated text of an element, with whitespace runs collapsed.
pub fn text_of(element: &ElementRef<'_>) -> String {
    collapse(&element.text().collect::<Vec<_>>().join(" "))
}

/// Text without whitespace normalisation.
pub fn raw_text_of(element: &ElementRef<'_>) -> String {
    element.text().collect::<Vec<_>>().join("")
}

/// Attribute value of an element.
pub fn attribute_of(element: &ElementRef<'_>, name: &str) -> Option<String> {
    element.value().attr(name).map(str::to_string)
}

/// Collapses whitespace runs and trims the result.
pub fn collapse(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_space && !out.is_empty() {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim().to_string()
}

/// Serialized, sanitized inner HTML of an element.
///
/// Script, style, iframe, object, embed and link elements are removed, inline
/// event handlers and `javascript:` URLs are stripped, and form controls are
/// left intact but without values. The result is data, never markup to execute.
pub fn inner_html(element: &ElementRef<'_>) -> String {
    let mut out = String::new();
    serialize_children(element, &mut out);
    sanitize_html(&out)
}

fn serialize_children(element: &ElementRef<'_>, out: &mut String) {
    for child in element.children() {
        match child.value() {
            scraper::Node::Text(text) => out.push_str(&escape_text(text)),
            _ => {
                if let Some(element) = ElementRef::wrap(child) {
                    serialize_node(&element, out);
                }
            }
        }
    }
}

fn serialize_node(node: &ElementRef<'_>, out: &mut String) {
    let value = node.value();
    let name = value.name();
    if is_dropped_element(name) {
        return;
    }
    out.push('<');
    out.push_str(name);
    for (attr_name, attr_value) in value.attrs() {
        if attr_name.starts_with("on") || attr_value.trim_start().starts_with("javascript:") {
            continue;
        }
        out.push(' ');
        out.push_str(attr_name);
        out.push_str("=\"");
        out.push_str(&escape_attr(attr_value));
        out.push('"');
    }
    out.push('>');
    if is_void_element(name) {
        return;
    }
    serialize_children(node, out);
    out.push_str("</");
    out.push_str(name);
    out.push('>');
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn is_dropped_element(name: &str) -> bool {
    matches!(
        name,
        "script" | "style" | "noscript" | "iframe" | "object" | "embed" | "link" | "meta" | "base"
    )
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Removes dangerous constructs from an HTML fragment produced by the engine.
pub fn sanitize_html(html: &str) -> String {
    use std::sync::OnceLock;
    static BLOCKS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    static HANDLERS: OnceLock<regex::Regex> = OnceLock::new();
    static JS_URLS: OnceLock<regex::Regex> = OnceLock::new();

    let blocks = BLOCKS.get_or_init(|| {
        ["script", "style", "iframe", "object", "embed"]
            .iter()
            .map(|tag| {
                regex::Regex::new(&format!(r"(?is)<{tag}\b[^>]*>.*?</{tag}\s*>"))
                    .expect("static regex")
            })
            .collect()
    });
    let handlers = HANDLERS.get_or_init(|| {
        regex::Regex::new(r#"(?i)\s+on[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)"#)
            .expect("static regex")
    });
    let js_urls = JS_URLS.get_or_init(|| {
        regex::Regex::new(r#"(?i)(href|src)\s*=\s*("|')\s*javascript:[^"']*("|')"#)
            .expect("static regex")
    });

    let mut cleaned = html.to_string();
    for block in blocks {
        cleaned = block.replace_all(&cleaned, "").into_owned();
    }
    cleaned = handlers.replace_all(&cleaned, "").into_owned();
    cleaned = js_urls.replace_all(&cleaned, "$1=\"#\"").into_owned();
    cleaned
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ------------------------------------------------------------------ XPath

#[derive(Debug, Clone, PartialEq, Eq)]
enum Test {
    Any,
    Name(String),
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Predicate {
    Index(usize),
    Last,
    AttrExists(String),
    AttrEquals(String, String),
    AttrContains(String, String),
    TextEquals(String),
    TextContains(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    descendant: bool,
    test: Test,
    predicates: Vec<Predicate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Terminal {
    Attribute(String),
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Expr {
    absolute: bool,
    steps: Vec<Step>,
    terminal: Option<Terminal>,
}

impl Expr {
    fn parse(input: &str) -> Result<Self> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(AicrawlError::InvalidInput(
                "XPath expression is empty".into(),
            ));
        }
        if raw.contains('|') {
            return Err(AicrawlError::InvalidInput(
                "XPath unions (|) are not supported".into(),
            ));
        }
        if raw.contains("::") {
            return Err(AicrawlError::InvalidInput(format!(
                "XPath axes are not supported. {SUPPORTED_XPATH}"
            )));
        }

        let mut absolute = false;
        let mut descendant = false;
        let mut body = raw;
        if let Some(rest) = body.strip_prefix("//") {
            absolute = true;
            descendant = true;
            body = rest;
        } else if let Some(rest) = body.strip_prefix("./") {
            descendant = true;
            body = rest;
        } else if let Some(rest) = body.strip_prefix('/') {
            absolute = true;
            body = rest;
        }

        let mut segments: Vec<(bool, String)> = Vec::new();
        let mut current = String::new();
        let mut depth = 0usize;
        let mut quote: Option<char> = None;
        let mut iter = body.chars().peekable();

        while let Some(ch) = iter.next() {
            if let Some(active) = quote {
                current.push(ch);
                if ch == active {
                    quote = None;
                }
                continue;
            }
            match ch {
                '\'' | '"' => {
                    quote = Some(ch);
                    current.push(ch);
                }
                '[' => {
                    depth += 1;
                    current.push(ch);
                }
                ']' => {
                    depth = depth.saturating_sub(1);
                    current.push(ch);
                }
                '/' if depth == 0 => {
                    let text = current.trim().to_string();
                    if !text.is_empty() && text != "." {
                        segments.push((descendant, text));
                    }
                    current.clear();
                    descendant = iter.peek() == Some(&'/');
                    if descendant {
                        iter.next();
                    }
                }
                _ => current.push(ch),
            }
        }
        if quote.is_some() {
            return Err(AicrawlError::InvalidInput(
                "XPath expression has an unterminated string literal".into(),
            ));
        }
        if depth != 0 {
            return Err(AicrawlError::InvalidInput(
                "XPath expression has unbalanced predicates".into(),
            ));
        }
        let tail = current.trim().to_string();
        if tail.is_empty() {
            return Err(AicrawlError::InvalidInput(
                "XPath expression ends with '/'".into(),
            ));
        }
        if tail != "." {
            segments.push((descendant, tail));
        }

        // A leading "//" arrives as an empty first segment: treat it as absolute
        // descendant-or-self instead of a step.
        let mut steps: Vec<Step> = Vec::new();
        let mut terminal: Option<Terminal> = None;
        let count = segments.len();
        for (index, (descendant, segment)) in segments.into_iter().enumerate() {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }
            if segment.starts_with('@') {
                if index + 1 != count {
                    return Err(AicrawlError::InvalidInput(
                        "an '@attribute' term must be the last part of an XPath".into(),
                    ));
                }
                let name = segment.trim_start_matches('@').trim();
                if name.is_empty() {
                    return Err(AicrawlError::InvalidInput(
                        "XPath attribute name is empty".into(),
                    ));
                }
                terminal = Some(Terminal::Attribute(name.to_string()));
                continue;
            }
            if segment == "text()" {
                if index + 1 != count {
                    return Err(AicrawlError::InvalidInput(
                        "a 'text()' term must be the last part of an XPath".into(),
                    ));
                }
                terminal = Some(Terminal::Text);
                continue;
            }
            steps.push(parse_step(descendant, segment)?);
        }
        if steps.is_empty() {
            return Err(AicrawlError::InvalidInput(
                "XPath expression contains no location steps".into(),
            ));
        }
        Ok(Self {
            absolute,
            steps,
            terminal,
        })
    }

    fn run<'a>(&self, scope: &ElementRef<'a>) -> Vec<Node<'a>> {
        let mut current: Vec<ElementRef<'a>> = vec![*scope];
        let mut matched: Vec<ElementRef<'a>> = Vec::new();

        for (index, step) in self.steps.iter().enumerate() {
            let first = index == 0;
            let mut next: Vec<ElementRef<'a>> = Vec::new();
            for base in &current {
                if first && !step.descendant {
                    if self.absolute {
                        // `/html/body`: match the root element itself.
                        let mut candidates = vec![*base];
                        apply_predicates(&mut candidates, step);
                        next.extend(candidates);
                    } else {
                        let mut candidates: Vec<ElementRef<'a>> =
                            base.children().filter_map(ElementRef::wrap).collect();
                        keep_matching(&mut candidates, step);
                        apply_predicates(&mut candidates, step);
                        next.extend(candidates);
                    }
                } else if first {
                    let mut candidates: Vec<ElementRef<'a>> = Vec::new();
                    if matches_test(*base, &step.test) {
                        candidates.push(*base);
                    }
                    collect_descendants(*base, &mut candidates);
                    keep_matching(&mut candidates, step);
                    apply_predicates(&mut candidates, step);
                    next.extend(candidates);
                } else if step.descendant {
                    let mut candidates = Vec::new();
                    collect_descendants(*base, &mut candidates);
                    keep_matching(&mut candidates, step);
                    apply_predicates(&mut candidates, step);
                    next.extend(candidates);
                } else {
                    let mut candidates: Vec<ElementRef<'a>> =
                        base.children().filter_map(ElementRef::wrap).collect();
                    keep_matching(&mut candidates, step);
                    apply_predicates(&mut candidates, step);
                    next.extend(candidates);
                }
            }
            current = next;
            if current.is_empty() {
                matched = Vec::new();
                break;
            }
            matched = current.clone();
        }

        match &self.terminal {
            None => matched.into_iter().map(Node::Element).collect(),
            Some(Terminal::Attribute(name)) => matched
                .into_iter()
                .filter_map(|element| attribute_of(&element, name))
                .map(Node::Attribute)
                .collect(),
            Some(Terminal::Text) => matched
                .into_iter()
                .map(|element| Node::Text(text_of(&element)))
                .collect(),
        }
    }
}

fn parse_step(descendant: bool, segment: &str) -> Result<Step> {
    let mut name_end = segment.len();
    let mut predicates: Vec<Predicate> = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;

    for (index, ch) in segment.char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '[' if depth == 0 => {
                if name_end == segment.len() {
                    name_end = index;
                }
                depth += 1;
            }
            ']' if depth == 1 => {
                depth = 0;
                let start = segment[..index]
                    .rfind('[')
                    .map(|position| position + 1)
                    .unwrap_or(0);
                predicates.push(parse_predicate(&segment[start..index])?);
            }
            ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if depth != 0 {
        return Err(AicrawlError::InvalidInput(format!(
            "unbalanced predicate in XPath step '{segment}'"
        )));
    }

    let name = segment[..name_end].trim();
    if name.is_empty() {
        return Err(AicrawlError::InvalidInput(format!(
            "unsupported XPath step '{segment}'"
        )));
    }
    let test = match name {
        "*" => Test::Any,
        "text()" => Test::Text,
        other => {
            if other
                .chars()
                .any(|ch| !(ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ':'))
            {
                return Err(AicrawlError::InvalidInput(format!(
                    "unsupported XPath step '{segment}'. {SUPPORTED_XPATH}"
                )));
            }
            Test::Name(other.to_ascii_lowercase())
        }
    };
    Ok(Step {
        descendant,
        test,
        predicates,
    })
}

fn parse_predicate(body: &str) -> Result<Predicate> {
    let body = body.trim();
    if body.is_empty() {
        return Err(AicrawlError::InvalidInput("empty XPath predicate".into()));
    }
    if body == "last()" {
        return Ok(Predicate::Last);
    }
    if let Ok(index) = body.parse::<usize>() {
        if index == 0 {
            return Err(AicrawlError::InvalidInput(
                "XPath positions are 1-based".into(),
            ));
        }
        return Ok(Predicate::Index(index));
    }
    if let Some(rest) = body.strip_prefix("contains(") {
        let inner = rest
            .strip_suffix(')')
            .ok_or_else(|| AicrawlError::InvalidInput("unterminated contains()".into()))?;
        let (subject, value) = split_args(inner)?;
        if subject.eq_ignore_ascii_case("text()") {
            return Ok(Predicate::TextContains(value));
        }
        let attribute = subject.strip_prefix('@').ok_or_else(|| {
            AicrawlError::InvalidInput(format!("unsupported contains() subject '{subject}'"))
        })?;
        return Ok(Predicate::AttrContains(attribute.to_string(), value));
    }
    if let Some(rest) = body.strip_prefix("text()") {
        let value = rest
            .trim_start()
            .strip_prefix('=')
            .ok_or_else(|| AicrawlError::InvalidInput(format!("unsupported predicate '{body}'")))?;
        return Ok(Predicate::TextEquals(unquote(value.trim())?));
    }
    if let Some(rest) = body.strip_prefix('@') {
        match rest.split_once('=') {
            Some((name, value)) => Ok(Predicate::AttrEquals(
                name.trim().to_string(),
                unquote(value.trim())?,
            )),
            None => Ok(Predicate::AttrExists(rest.trim().to_string())),
        }
    } else {
        Err(AicrawlError::InvalidInput(format!(
            "unsupported XPath predicate '[{body}]'. {SUPPORTED_XPATH}"
        )))
    }
}

fn split_args(inner: &str) -> Result<(String, String)> {
    let mut quote: Option<char> = None;
    for (index, ch) in inner.char_indices() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            ',' => {
                let subject = inner[..index].trim().to_string();
                let value = unquote(inner[index + 1..].trim())?;
                return Ok((subject, value));
            }
            _ => {}
        }
    }
    Err(AicrawlError::InvalidInput(
        "contains() requires two arguments".into(),
    ))
}

fn unquote(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.chars().next().unwrap_or(' ');
        let last = trimmed.chars().last().unwrap_or(' ');
        if (first == '\'' && last == '\'') || (first == '"' && last == '"') {
            return Ok(trimmed[1..trimmed.len() - 1].to_string());
        }
    }
    Err(AicrawlError::InvalidInput(format!(
        "expected a quoted string in XPath predicate, found '{trimmed}'"
    )))
}

fn matches_test(element: ElementRef<'_>, test: &Test) -> bool {
    match test {
        Test::Any => true,
        Test::Text => false,
        Test::Name(name) => element.value().name().eq_ignore_ascii_case(name),
    }
}

fn keep_matching(elements: &mut Vec<ElementRef<'_>>, step: &Step) {
    elements.retain(|element| matches_test(*element, &step.test));
}

fn apply_predicates(elements: &mut Vec<ElementRef<'_>>, step: &Step) {
    for predicate in &step.predicates {
        match predicate {
            Predicate::Index(index) => {
                let position = index.saturating_sub(1);
                let kept = elements.get(position).copied();
                elements.clear();
                if let Some(element) = kept {
                    elements.push(element);
                }
            }
            Predicate::Last => {
                let last = elements.last().copied();
                elements.clear();
                if let Some(element) = last {
                    elements.push(element);
                }
            }
            Predicate::AttrExists(name) => {
                elements.retain(|element| element.value().attr(name).is_some())
            }
            Predicate::AttrEquals(name, expected) => {
                elements.retain(|element| element.value().attr(name) == Some(expected.as_str()))
            }
            Predicate::AttrContains(name, needle) => elements.retain(|element| {
                element
                    .value()
                    .attr(name)
                    .map(|value| value.contains(needle.as_str()))
                    .unwrap_or(false)
            }),
            Predicate::TextEquals(expected) => {
                elements.retain(|element| text_of(element) == *expected)
            }
            Predicate::TextContains(needle) => {
                elements.retain(|element| text_of(element).contains(needle.as_str()))
            }
        }
    }
}

fn collect_descendants<'a>(element: ElementRef<'a>, out: &mut Vec<ElementRef<'a>>) {
    for child in element.children().filter_map(ElementRef::wrap) {
        out.push(child);
        collect_descendants(child, out);
    }
}

/// Serialized outer HTML of the whole document (used by tests and diagnostics).
pub fn document_html(document: &Html) -> String {
    document.html()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> Html {
        Html::parse_document(
            r#"<html><head><title>Shop</title></head><body>
                 <div id="main" class="page">
                   <div class="product-card"><h2 class="title">Widget</h2><span class="price">$19.99</span><a href="/p/1">Details</a></div>
                   <div class="product-card"><h2 class="title">Gadget</h2><span class="price">$29.00</span><a href="/p/2">Details</a></div>
                 </div>
                 <ul><li>one</li><li>two</li><li>three</li></ul>
                 <a class="next" href="/page/2">Next</a>
                 <script>alert('x')</script>
               </body></html>"#,
        )
    }

    fn body() -> ElementRef<'static> {
        // Leaks the document for the duration of the test process; tests only.
        let document: &'static Html = Box::leak(Box::new(document()));
        document.root_element()
    }

    #[test]
    fn css_selects_and_reads_text_and_attributes() {
        let root = body();
        let cards = select_css(&root, ".product-card").unwrap();
        assert_eq!(cards.len(), 2);
        let first = cards[0].element().unwrap();
        let titles = select_css(&first, ".title").unwrap();
        assert_eq!(
            titles[0].element().unwrap().text().collect::<String>(),
            "Widget"
        );
        assert_eq!(
            attribute_of(&titles[0].element().unwrap(), "class").as_deref(),
            Some("title")
        );
        assert!(select_css(&root, "div[").is_err());
    }

    #[test]
    fn xpath_supports_paths_predicates_and_terminals() {
        let root = body();
        let items = select_xpath(&root, "//div[@class='product-card']").unwrap();
        assert_eq!(items.len(), 2);
        let second = items[1].element().unwrap();
        assert_eq!(text_of(&second), "Gadget $29.00 Details");

        let hrefs = select_xpath(&root, "//div[@class='product-card']/a/@href").unwrap();
        let hrefs: Vec<&str> = hrefs.iter().filter_map(Node::literal).collect();
        assert_eq!(hrefs, vec!["/p/1", "/p/2"]);

        let first_title =
            select_xpath(&root, "//div[@class='product-card'][1]//h2/text()").unwrap();
        assert_eq!(first_title[0].literal().unwrap(), "Widget");

        let last_li = select_xpath(&root, "//li[last()]/text()").unwrap();
        assert_eq!(last_li[0].literal().unwrap(), "three");

        let next = select_xpath(&root, "//a[contains(@class,'next')]/@href").unwrap();
        assert_eq!(next[0].literal().unwrap(), "/page/2");

        let id = select_xpath(&root, "//*[@id='main']").unwrap();
        assert_eq!(id.len(), 1);

        let html_body = select_xpath(&root, "/html/body").unwrap();
        assert_eq!(html_body.len(), 1);
    }

    #[test]
    fn xpath_rejects_unsupported_syntax() {
        for expression in ["//div[", "//div/@a/b", "//div|//span", "//x::y", "//li[0]"] {
            assert!(validate_xpath(expression).is_err(), "{expression}");
        }
        assert!(validate_xpath("//div[@class='x']").is_ok());
    }

    #[test]
    fn inner_html_is_sanitized() {
        let root = body();
        let main = select_css(&root, "#main").unwrap()[0].element().unwrap();
        let html = inner_html(&main);
        assert!(html.contains("product-card"));
        assert!(!html.contains("<script"));

        let raw = r#"<div onclick="alert(1)"><script>x()</script><a href="javascript:void(0)">x</a></div>"#;
        let cleaned = sanitize_html(raw);
        assert!(!cleaned.contains("onclick"));
        assert!(!cleaned.contains("<script"));
        assert!(!cleaned.contains("javascript:"));
    }

    #[test]
    fn whitespace_is_collapsed() {
        assert_eq!(collapse("  a \n\t b  "), "a b");
        assert_eq!(collapse(""), "");
    }
}
