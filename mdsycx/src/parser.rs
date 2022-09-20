//! Parse MD with custom extensions.

use std::borrow::Cow;

use pulldown_cmark::{CodeBlockKind, Event as MdEvent, Tag};
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
    let md_parser = pulldown_cmark::Parser::new(input);
    let md_events = md_parser.collect::<Vec<_>>();
    let mut events = Vec::new();

    for e in md_events {
        match e {
            MdEvent::Start(tag) => start_tag(tag, &mut events),
            MdEvent::End(tag) => end_tag(tag, &mut events),
            MdEvent::Text(text) => events.push(Event::Text(text.into())),
            MdEvent::Code(text) => {
                events.push(Event::Start("code".into()));
                events.push(Event::Text(text.into()));
                events.push(Event::End);
            }
            MdEvent::Html(_) => todo!("inline html"),
            MdEvent::FootnoteReference(_) => todo!("footnote reference"),
            MdEvent::SoftBreak => {} // Do nothing.
            MdEvent::HardBreak => {
                events.push(Event::Start("br".into()));
                events.push(Event::End);
            }
            MdEvent::Rule => {
                events.push(Event::Start("hr".into()));
                events.push(Event::End);
            }
            MdEvent::TaskListMarker(_) => todo!("task list marker"),
        }
    }

    BodyRes { events }
}

fn start_tag<'a>(tag: Tag<'a>, events: &mut Vec<Event<'a>>) {
    match tag {
        Tag::Paragraph => events.push(Event::Start("p".into())),
        Tag::Heading(lv, _, _) => events.push(Event::Start(lv.to_string().into())),
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
        Tag::FootnoteDefinition(_) => todo!("footnote definition"),
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
        Tag::FootnoteDefinition(_) => todo!("footnote definition"),
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