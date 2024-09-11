//! Parse MD with custom extensions.

use std::borrow::Cow;

use pulldown_cmark::html::push_html;
use pulldown_cmark::Options;
use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
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
pub struct ParseRes<'a, T = ()> {
    /// The parsed MD front matter. If no front matter was present, this has a value of `None`.
    pub front_matter: T,
    /// The parsed file. This should be passed when rendering the Markdown with Sycamore.
    pub body: BodyRes<'a>,
}

/// The parsed markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyRes<'a> {
    #[serde(borrow)]
    pub(crate) events: Vec<Event<'a>>,
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
pub fn parse<'de, T>(input: &'de str) -> Result<ParseRes<'de, T>, ParseError>
where
    T: Deserialize<'de>,
{
    let input = input.trim();
    if let Some(("", rest)) = input.split_once("---") {
        // Parse front matter.
        if let Some((front_matter_str, body_str)) = rest.split_once("---") {
            let front_matter = serde_yaml::from_str(front_matter_str)?;

            Ok(ParseRes {
                front_matter,
                body: parse_md(body_str),
            })
        } else {
            Err(ParseError::MissingFrontMatterEndDelimiter)
        }
    } else {
        // Try to parse front matter from an empty string.
        let front_matter = serde_yaml::from_str::<T>("")?;
        Ok(ParseRes {
            front_matter,
            body: parse_md(input),
        })
    }
}

/// Parse Markdown into structured events.
fn parse_md(input: &str) -> BodyRes {
    let md_parser = pulldown_cmark::Parser::new_ext(input, Options::all()).peekable();
    let mut html = String::new();
    push_html(&mut html, md_parser);

    let mut events = Vec::new();
    parse_html(&html, &mut events);

    BodyRes { events }
}

fn parse_html(input: &str, events: &mut Vec<Event>) {
    let mut reader = Reader::from_str(input);

    // Keep track of the element depth. If the depth is not 0 when parsing is finished, that means
    // that the HTML was malformed and we need to emit extra End tags.
    let mut depth = 0;

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
                depth += 1;
            }
            Ok(XmlEvent::End(_)) => {
                if depth != 0 {
                    events.push(Event::End);
                    depth -= 1;
                } else {
                    #[cfg(target_arch = "wasm32")]
                    web_sys::console::warn_1(&"html tags are not balanced".into());
                    #[cfg(not(target_arch = "wasm32"))]
                    eprintln!("html tags are not balanced");
                }
            }
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
            Ok(XmlEvent::Eof) => break,
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

    if depth > 0 {
        #[cfg(target_arch = "wasm32")]
        web_sys::console::warn_1(&"html tags are not balanced".into());
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("html tags are not balanced");

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
        let actual = parse_md(input);
        expect.assert_eq(&format!("{:?}", actual.events));
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
                r#"[Start("h1"), Text("My heading"), End, Text("\n"), Start("h2"), Text("My subtitle"), End, Text("\n"), Start("p"), Text("My text"), End, Text("\n"), Start("ul"), Text("\n"), Start("li"), Text("List item"), End, Text("\n"), End, Text("\n")]"#
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
}
