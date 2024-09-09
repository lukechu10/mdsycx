//! Sycamore bindings for rendering MD with components.

use std::rc::Rc;
use std::{borrow::Cow, collections::HashMap};

use sycamore::prelude::*;
use sycamore::web::{ViewHtmlNode, ViewNode};

use crate::{BodyRes, Event, FromMd};

type MdComponentProps = (Vec<(String, String)>, View);

/// A type-erased component that can be used from Markdown.
type MdComponent = Rc<dyn Fn(MdComponentProps) -> View + 'static>;

/// Convert a Sycamore component into a type-erased component. The props need to implement
/// [`FromMd`].
fn into_type_erased_component<F, Props>(f: F) -> impl Fn(MdComponentProps) -> View
where
    F: Fn(Props) -> View,
    Props: FromMd,
{
    move |(props_serialized, children)| {
        let mut props = Props::new_prop_default();
        for (name, value) in props_serialized {
            if let Err(err) = props.set_prop(&name, &value) {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::warn_1(&format!("error setting prop {name}: {err}").into());
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("error setting prop {name}: {err}");
            }
        }
        props.set_children(children);
        f(props)
    }
}

/// A map from component names to component functions.
pub struct ComponentMap {
    map: HashMap<String, MdComponent>,
}

impl ComponentMap {
    /// Create a new empty [`ComponentMap`].
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    /// Adds a mapping between a component name and its actual implementation.
    pub fn with<F, Props>(mut self, name: &'static str, f: F) -> Self
    where
        F: Fn(Props) -> View + 'static,
        Props: FromMd,
    {
        self.map
            .insert(name.to_string(), Rc::new(into_type_erased_component(f)));
        self
    }
}

impl Default for ComponentMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Props for [`MDSycX`].
#[derive(Props)]
pub struct MdSycXProps {
    body: BodyRes<'static>,
    #[prop(default)]
    components: ComponentMap,
}

/// Renders your Sycamore augmented markdown.
#[component]
pub fn MDSycX(props: MdSycXProps) -> View {
    let events = props.body.events;
    events_to_view(events, props.components)
}

enum TagType {
    Element(Cow<'static, str>),
    Component(MdComponent),
}

fn events_to_view(events: Vec<Event<'static>>, components: ComponentMap) -> View {
    // A stack of fragments. The bottom fragment is the view that is returned. Subsequent fragments
    // are those in nested elements.
    let mut fragments_stack: Vec<Vec<View>> = vec![Vec::new()];
    // Attributes that should be added when end tag is reached.
    let mut attr_stack: Vec<Vec<(String, String)>> = vec![Vec::new()];
    // Elements that should be constructed when the end tag is reached.
    let mut tag_stack = Vec::new();
    for ev in events {
        match ev {
            Event::Start(tag) => {
                fragments_stack.push(Vec::new());
                attr_stack.push(Vec::new());
                // Check if a component is registered for the tag.
                if let Some(component) = components.map.get(&tag.to_string()).cloned() {
                    // Render the component instead of the element.
                    tag_stack.push(TagType::Component(component))
                } else {
                    tag_stack.push(TagType::Element(tag));
                }
            }
            Event::End => {
                let tag = tag_stack.pop().expect("events are not balanced");
                match tag {
                    TagType::Element(tag) => {
                        let mut node = sycamore::web::HtmlNode::create_element(tag);
                        // Add children to node.
                        let children = fragments_stack.pop().expect("events are not balanced");
                        node.append_view(children.into());
                        // Add attributes to node.
                        let attributes = attr_stack.pop().expect("events are not balanced");
                        for (name, value) in attributes {
                            node.set_attribute(name.into(), value.into());
                        }
                        fragments_stack
                            .last_mut()
                            .expect("should always have at least one fragment on stack")
                            .push(node.into());
                    }
                    TagType::Component(component) => {
                        // Collect attributes into serde_json::Value.
                        let attributes = attr_stack.pop().expect("events are not balanced");
                        let children = fragments_stack.pop().expect("events are not balanced");
                        let node = component((attributes, children.into()));
                        fragments_stack
                            .last_mut()
                            .expect("should always have at least one fragment on stack")
                            .push(node);
                    }
                }
            }
            Event::Attr(name, value) => {
                attr_stack
                    .last_mut()
                    .expect("cannot set attributes without an element")
                    .push((name.to_string(), value.to_string()));
            }
            Event::Text(text) => {
                let node: View = text.to_string().into();
                fragments_stack
                    .last_mut()
                    .expect("should always have at least one fragment on stack")
                    .push(node)
            }
        }
    }

    if fragments_stack.len() != 1 {
        panic!("fragment stack is not balanced");
    }

    fragments_stack.into_iter().next().unwrap().into()
}
