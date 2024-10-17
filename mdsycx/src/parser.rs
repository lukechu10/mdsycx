//! Parse MD with custom extensions.

use std::collections::HashMap;

use pulldown_cmark::html::push_html;
use pulldown_cmark::Options;
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use sycamore::web::console_warn;
use thiserror::Error;

/// An error from parsing mdsycx.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The front matter section was encountered but could not find ending delimiter.
    ///
    /// This means that a `---` was found at the top of the file but the ending `---` could not be
    /// found.
    #[error("front matter is missing end delimiter")]
    MissingFrontMatterEndDelimiter,
    /// Could not deserialize the front matter into the type. Deserialization uses [`serde_yaml`].
    #[error("could not parse yaml")]
    DeserializeError(#[from] serde_yaml::Error),
}

/// The result of parsing mdsycx.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseRes<T = ()> {
    /// The parsed MD front matter. If no front matter was present, this has a value of `None`.
    pub front_matter: T,
    /// An outline of the document. Contains the text and the ids of all the headings found in the
    /// document.
    pub headings: Vec<OutlineHeading>,
    /// The parsed file. This should be passed when rendering the Markdown with Sycamore.
    pub body: BodyRes,
}

/// A heading in the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineHeading {
    /// The anchor associated with the heading.
    pub id: String,
    /// The text of the heading.
    pub text: String,
    /// The level of the heading.
    pub level: u32,
}

/// The parsed markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyRes {
    pub(crate) events: Vec<Event>,
}

/// Tree events, or "instructions" that can be serialized and rendered with Sycamore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// Create a new tag.
    Start(String),
    /// End of new tag.
    End,
    /// Add an attribute to the current tag.
    Attr(String, String),
    /// Text node.
    Text(String),
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
            let front_matter = serde_yaml::from_str(front_matter_str)?;

            let (headings, body) = parse_md(body_str);
            Ok(ParseRes {
                front_matter,
                headings,
                body,
            })
        } else {
            Err(ParseError::MissingFrontMatterEndDelimiter)
        }
    } else {
        // Try to parse front matter from an empty string.
        let front_matter = serde_yaml::from_str::<T>("")?;
        let (headings, body) = parse_md(input);
        Ok(ParseRes {
            front_matter,
            headings,
            body,
        })
    }
}

/// Parse Markdown into structured events.
fn parse_md(input: &str) -> (Vec<OutlineHeading>, BodyRes) {
    let md_parser = pulldown_cmark::Parser::new_ext(input, Options::all()).peekable();
    let mut html = String::new();
    push_html(&mut html, md_parser);

    let mut headings = Vec::new();
    let mut events = Vec::new();
    parse_html(&html, &mut headings, &mut events);

    (headings, BodyRes { events })
}

#[derive(Debug, Default)]
struct SlugState {
    ids: HashMap<String, u32>,
}

impl SlugState {
    pub fn slugify(&mut self, text: &str) -> String {
        let slug = text
            .to_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
            .trim_matches('-')
            .to_string();

        let count = self.ids.entry(slug.clone()).or_insert(0);
        *count += 1;

        if *count > 1 {
            format!("{}-{}", slug, count)
        } else {
            slug
        }
    }
}

fn parse_html(input: &str, headings: &mut Vec<OutlineHeading>, events: &mut Vec<Event>) {
    let mut reader = Reader::from_str(input);

    // Keep track of the element depth. If the depth is not 0 when parsing is finished, that means
    // that the HTML was malformed and we need to emit extra End tags.
    let mut depth = 0;

    let mut slugger = SlugState::default();
    let mut heading_title = None;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(start)) => {
                let tag = start.name().0;
                // Check if this is the start of a heading. If so, initialize `heading_title`.
                if tag.len() == 2 && tag[0] == b'h' && tag[1].is_ascii_digit() {
                    heading_title = Some(String::new());
                }

                let tag = String::from_utf8(tag.to_vec()).unwrap();

                events.push(Event::Start(tag));
                for attr in start.html_attributes().with_checks(false).flatten() {
                    events.push(Event::Attr(
                        String::from_utf8(attr.key.0.to_vec()).unwrap(),
                        String::from_utf8(attr.value.to_vec()).unwrap(),
                    ));
                }
                depth += 1;
            }
            Ok(XmlEvent::End(tag)) => {
                // Check if this is the end of a heading. If so, set `heading_title` back to `None`
                // and slug the title.
                if tag.len() == 2 && tag[0] == b'h' && tag[1].is_ascii_digit() {
                    if let Some(title) = heading_title.take() {
                        let id = slugger.slugify(&title);
                        events.push(Event::Attr("id".to_string(), id.clone()));
                        headings.push(OutlineHeading {
                            id,
                            text: title,
                            level: (tag[1] - b'0') as u32,
                        });
                    }
                }

                if depth != 0 {
                    events.push(Event::End);
                    depth -= 1;
                } else {
                    console_warn!("html tags are not balanced");
                }
            }
            Ok(XmlEvent::Empty(start)) => {
                let tag = String::from_utf8(start.name().0.to_vec()).unwrap();
                events.push(Event::Start(tag));
                for attr in start.html_attributes().with_checks(false).flatten() {
                    events.push(Event::Attr(
                        String::from_utf8(attr.key.0.to_vec()).unwrap(),
                        String::from_utf8(attr.value.to_vec()).unwrap(),
                    ));
                }
                events.push(Event::End);
            }
            Ok(XmlEvent::Text(text)) => {
                let text = text.unescape().unwrap().to_string();
                if let Some(title) = heading_title.as_mut() {
                    title.push_str(&text);
                }
                events.push(Event::Text(text))
            }
            Ok(XmlEvent::Eof) => break,
            Err(e) => {
                console_warn!("html parsing error: {e}");
            }
            _ => {}
        }

        buf.clear();
    }

    if depth > 0 {
        console_warn!("html tags are not balanced");

        for _ in 0..depth {
            events.push(Event::End);
        }
    }
}

#[cfg(test)]
mod tests {
    use expect_test::{expect, Expect};

    use super::*;

    fn check(input: &str, expect: Expect) {
        let (_headings, body) = parse_md(input);
        expect.assert_eq(&format!("{:?}", body.events));
    }

    #[test]
    fn parse_empty() {
        check(r#""#, expect!["[]"]);
    }

    #[test]
    fn parse_md_text() {
        check(
            r#"
Hello World!
Goodbye World!"#,
            expect![[r#"[Start("p"), Text("Hello World!\nGoodbye World!"), End, Text("\n")]"#]],
        );
    }

    #[test]
    fn parse_md_features() {
        check(
            r#"
# My heading
## My subtitle
My text

- List item"#,
            expect![[
                r#"[Start("h1"), Text("My heading"), Attr("id", "my-heading"), End, Text("\n"), Start("h2"), Text("My subtitle"), Attr("id", "my-subtitle"), End, Text("\n"), Start("p"), Text("My text"), End, Text("\n"), Start("ul"), Text("\n"), Start("li"), Text("List item"), End, Text("\n"), End, Text("\n")]"#
            ]],
        );
    }

    #[test]
    fn parse_html_block() {
        check(
            r#"<div id="test">A div!</div>"#,
            expect![[r#"[Start("div"), Attr("id", "test"), Text("A div!"), End]"#]],
        );
    }

    #[test]
    fn parse_html_self_closing_tag() {
        check(r#"<br />"#, expect![[r#"[Start("br"), End]"#]]);
    }

    #[test]
    fn parse_nested_html() {
        check(
            r#"<div><p>Test</p></div>"#,
            expect![[r#"[Start("div"), Start("p"), Text("Test"), End, End]"#]],
        );
    }

    #[test]
    fn parse_multiline_html() {
        check(
            r#"
<div>
    <p>Nested</p>
    Text
</div>"#,
            expect![[
                r#"[Start("div"), Text("\n    "), Start("p"), Text("Nested"), End, Text("\n    Text\n"), End]"#
            ]],
        );
    }

    #[test]
    fn parse_inline_html_in_text() {
        check(
            r#"<i>Some inline</i> text"#,
            expect![[
                r#"[Start("p"), Start("i"), Text("Some inline"), End, Text(" text"), End, Text("\n")]"#
            ]],
        );
        check(
            r#"Some inline <em>text</em>"#,
            expect![[
                r#"[Start("p"), Text("Some inline "), Start("em"), Text("text"), End, End, Text("\n")]"#
            ]],
        );
    }

    #[test]
    fn parse_inline_nested_html() {
        check(
            r#"Some inline <span><i>text</i></span>"#,
            expect![[
                r#"[Start("p"), Text("Some inline "), Start("span"), Start("i"), Text("text"), End, End, End, Text("\n")]"#
            ]],
        );
    }

    #[test]
    fn check_slug() {
        let mut state = SlugState::default();
        assert_eq!(state.slugify("Hello World"), "hello-world");
        assert_eq!(state.slugify("Hello World"), "hello-world-2");
        assert_eq!(state.slugify("hello world"), "hello-world-3");
        assert_eq!(state.slugify("hello-world"), "hello-world-4");
        assert_eq!(state.slugify("hello world!"), "hello-world-5");

        assert_eq!(state.slugify("Goodbye World"), "goodbye-world");
    }

    #[test]
    fn check_duplicated_heading() {
        check(
            r#"
# Hello World
## Hello World!"#,
            expect![[
                r#"[Start("h1"), Text("Hello World"), Attr("id", "hello-world"), End, Text("\n"), Start("h2"), Text("Hello World!"), Attr("id", "hello-world-2"), End, Text("\n")]"#
            ]],
        )
    }
}
