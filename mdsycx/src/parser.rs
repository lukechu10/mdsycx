//! Parse MD with custom extensions.

use std::borrow::Cow;
use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, CowStr, Event as MdEvent, Options, Tag};
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An error from parsing MDSycX.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("front matter is missing end delimiter")]
    MissingFrontMatterEndDelimiter,
    #[error("could not parse yaml")]
    DeserializeError(#[from] serde_yaml::Error),
}

/// The result of parsing MDSycX.
pub struct ParseRes<'a, T> {
    /// The parsed MD front matter. If no front matter was present, this has a value of `None`.
    pub front_matter: Option<T>,
    /// The parsed file. This should be passed when rendering the Markdown with Sycamore.
    pub body: BodyRes<'a>,
}

#[derive(Serialize, Deserialize)]
pub struct BodyRes<'a> {
    #[serde(borrow)]
    pub events: Vec<Event<'a>>,
}

/// Tree events, or "instructions" that can be serialized and rendered with Sycamore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event<'a> {
    /// Create a new tag.
    Start(Cow<'a, str>),
    /// End of new tag.
    End,
    /// Add an attribute to the current tag.
    Attr(Cow<'a, str>, Cow<'a, str>),
    /// Text node.
    Text(Cow<'a, str>),
}

/// Parse the the markdown document, including the front matter. The front matter is the metadata of
/// the document. It should be at the top of the file and surrounded by `---` characters.
pub fn parse<'de, T>(input: &'de str) -> Result<ParseRes<T>, ParseError>
where
    T: Deserialize<'de>,
{
    let input = input.trim();
    if let Some(("", rest)) = input.split_once("---") {
        // Parse front matter.
        if let Some((front_matter_str, body_str)) = rest.split_once("---") {
            let front_matter = Some(serde_yaml::from_str(front_matter_str)?);

            Ok(ParseRes {
                front_matter,
                body: parse_md(body_str),
            })
        } else {
            Err(ParseError::MissingFrontMatterEndDelimiter)
        }
    } else {
        Ok(ParseRes {
            front_matter: None,
            body: parse_md(input),
        })
    }
}

/// Parse Markdown into structured events.
fn parse_md(input: &str) -> BodyRes {
    let mut md_parser = pulldown_cmark::Parser::new_ext(input, Options::all()).peekable();

    let mut events = Vec::new();

    let mut numbers = HashMap::new();

    while let Some(e) = md_parser.next() {
        match e {
            MdEvent::Start(tag) => start_tag(tag, &mut events, &mut numbers),
            MdEvent::End(tag) => end_tag(tag, &mut events),
            MdEvent::Text(text) => events.push(Event::Text(text.into())),
            MdEvent::Code(text) => {
                events.push(Event::Start("code".into()));
                events.push(Event::Text(text.into()));
                events.push(Event::End);
            }
            MdEvent::Html(html) => {
                let mut html = html.to_string();
                // If next events are also html events, collect them together.
                while let Some(next_html) = md_parser.next_if(|e| matches!(e, MdEvent::Html(_))) {
                    match next_html {
                        MdEvent::Html(next_html) => html.push_str(&next_html),
                        _ => unreachable!(),
                    }
                }
                parse_html(&html, &mut events)
            }
            MdEvent::FootnoteReference(name) => {
                let len = numbers.len() + 1;
                events.push(Event::Start("sup".into()));
                events.push(Event::Attr("class".into(), "footnote-reference".into()));
                events.push(Event::Start("a".into()));
                let href = numbers.entry(name).or_insert(len);
                events.push(Event::Attr("href".into(), href.to_string().into()));
                events.push(Event::Text(href.to_string().into()));
                events.push(Event::End);
                events.push(Event::End);
            }
            MdEvent::SoftBreak => events.push(Event::Text(" ".into())),
            MdEvent::HardBreak => {
                events.push(Event::Start("br".into()));
                events.push(Event::End);
            }
            MdEvent::Rule => {
                events.push(Event::Start("hr".into()));
                events.push(Event::End);
            }
            MdEvent::TaskListMarker(checked) => {
                events.push(Event::Start("input".into()));
                events.push(Event::Attr("disabled".into(), "".into()));
                events.push(Event::Attr("type".into(), "checkbox".into()));
                if checked {
                    events.push(Event::Attr("checked".into(), "".into()));
                }
                events.push(Event::End);
            }
        }
    }

    BodyRes { events }
}

fn start_tag<'a>(
    tag: Tag<'a>,
    events: &mut Vec<Event<'a>>,
    numbers: &mut HashMap<CowStr<'a>, usize>,
) {
    match tag {
        Tag::Paragraph => events.push(Event::Start("p".into())),
        Tag::Heading(lv, id, class) => {
            events.push(Event::Start(lv.to_string().into()));
            if let Some(id) = id {
                events.push(Event::Attr("id".into(), id.into()));
            }
            let class = class.join(" ");
            if !class.is_empty() {
                events.push(Event::Attr("class".into(), class.into()));
            }
        }
        Tag::BlockQuote => events.push(Event::Start("blockquote".into())),
        Tag::CodeBlock(cb) => {
            events.push(Event::Start("pre".into()));
            events.push(Event::Start("code".into()));
            if let CodeBlockKind::Fenced(lang) = cb {
                if !lang.is_empty() {
                    events.push(Event::Attr(
                        "class".into(),
                        format!("language-{lang}").into(),
                    ));
                }
            }
        }
        Tag::List(list) => match list {
            Some(1) => events.push(Event::Start("ol".into())),
            Some(n) => {
                events.push(Event::Start("ol".into()));
                events.push(Event::Attr("start".into(), n.to_string().into()));
            }
            None => events.push(Event::Start("ul".into())),
        },
        Tag::Item => events.push(Event::Start("li".into())),
        Tag::FootnoteDefinition(name) => {
            events.push(Event::Start("div".into()));
            events.push(Event::Attr("class".into(), "footnote-definition".into()));
            events.push(Event::Attr("id".into(), name.clone().into()));

            events.push(Event::Start("sup".into()));
            events.push(Event::Attr(
                "class".into(),
                "footnote-definition-label".into(),
            ));

            let len = numbers.len() + 1;
            let number = numbers.entry(name).or_insert(len);
            events.push(Event::Text(number.to_string().into()));

            events.push(Event::End);
        }
        Tag::Table(_) => events.push(Event::Start("table".into())),
        Tag::TableHead => {
            events.push(Event::Start("thead".into()));
            events.push(Event::Start("tr".into()));
        }
        Tag::TableRow => events.push(Event::Start("tr".into())),
        // TODO: emit <th> in header and emit styles for alignment
        Tag::TableCell => events.push(Event::Start("td".into())),
        Tag::Emphasis => events.push(Event::Start("em".into())),
        Tag::Strong => events.push(Event::Start("strong".into())),
        Tag::Strikethrough => events.push(Event::Start("del".into())),
        Tag::Link(_, dest, title) => {
            events.push(Event::Start("a".into()));
            events.push(Event::Attr("href".into(), dest.into()));
            if !title.is_empty() {
                events.push(Event::Attr("title".into(), title.into()));
            }
        }
        Tag::Image(_, dest, title) => {
            events.push(Event::Start("img".into()));
            events.push(Event::Attr("src".into(), dest.into()));
            if !title.is_empty() {
                events.push(Event::Attr("title".into(), title.into()));
            }
            events.push(Event::End);
        }
    }
}

fn end_tag<'a>(tag: Tag<'a>, events: &mut Vec<Event<'a>>) {
    match tag {
        Tag::Paragraph | Tag::Heading(_, _, _) => events.push(Event::End),
        Tag::BlockQuote => events.push(Event::End),
        Tag::CodeBlock(_) => {
            events.push(Event::End);
            events.push(Event::End);
        }
        Tag::List(_) => events.push(Event::End),
        Tag::Item => events.push(Event::End),
        Tag::FootnoteDefinition(_) => events.push(Event::End),
        Tag::Table(_) => events.push(Event::End),
        Tag::TableHead => {
            events.push(Event::End);
            events.push(Event::End);
        }
        Tag::TableRow => events.push(Event::End),
        Tag::TableCell => events.push(Event::End),
        Tag::Emphasis => events.push(Event::End),
        Tag::Strong => events.push(Event::End),
        Tag::Strikethrough => events.push(Event::End),
        Tag::Link(_, _, _) => events.push(Event::End),
        Tag::Image(_, _, _) => unreachable!("handled in start_tag"),
    }
}

fn parse_html<'a>(input: &str, events: &mut Vec<Event<'a>>) {
    let mut reader = Reader::from_str(input);
    reader.trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(start)) => {
                events.push(Event::Start(
                    String::from_utf8(start.name().0.to_vec()).unwrap().into(),
                ));
                for attr in start.html_attributes().with_checks(false).flatten() {
                    events.push(Event::Attr(
                        String::from_utf8(attr.key.0.to_vec()).unwrap().into(),
                        String::from_utf8(attr.value.to_vec()).unwrap().into(),
                    ));
                }
            }
            Ok(XmlEvent::End(_)) => events.push(Event::End),
            Ok(XmlEvent::Empty(start)) => {
                events.push(Event::Start(
                    String::from_utf8(start.name().0.to_vec()).unwrap().into(),
                ));
                for attr in start.html_attributes().with_checks(false).flatten() {
                    events.push(Event::Attr(
                        String::from_utf8(attr.key.0.to_vec()).unwrap().into(),
                        String::from_utf8(attr.value.to_vec()).unwrap().into(),
                    ));
                }
                events.push(Event::End);
            }
            Ok(XmlEvent::Text(text)) => {
                events.push(Event::Text(text.unescape().unwrap().to_string().into()))
            }
            Ok(XmlEvent::Eof) => return,
            Err(e) => {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::warn_1(&format!("html parsing error: {e}").into());
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("html parsing error: {e}");
            }
            _ => {}
        }

        buf.clear();
    }
}
