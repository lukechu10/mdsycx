//! Sycamore bindings for rendering MD with components.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Map, Value};
use sycamore::prelude::*;
use sycamore::utils::render::insert;

use crate::{BodyRes, Event};

type TypeErasedComponent<G> = Box<dyn Fn(Scope, Value) -> View<G>>;

/// Convert a Sycamore component into a type-erased component. The props need to be deserializable
/// (with serde).
fn into_type_erased_component<G: Html, F, Props>(f: F) -> impl Fn(Scope, Value) -> View<G>
where
    F: Fn(Scope, Props) -> View<G>,
    Props: for<'de> Deserialize<'de>,
{
    move |cx, serialized| {
        let deserialized = serde_json::from_value(serialized).expect("could not deserialize prop");
        f(cx, deserialized)
    }
}

/// A map from component names to component functions.
pub struct ComponentMap<G: Html> {
    map: HashMap<String, TypeErasedComponent<G>>,
}

impl<G: Html> ComponentMap<G> {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    pub fn with<F, Props>(mut self, name: &'static str, f: F) -> Self
    where
        F: Fn(Scope, Props) -> View<G> + 'static,
        Props: for<'de> Deserialize<'de> + 'static,
    {
        self.map
            .insert(name.to_string(), Box::new(into_type_erased_component(f)));
        self
    }
}

impl<G: Html> Default for ComponentMap<G> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Prop)]
pub struct MdSycXProps<'a, G: Html> {
    body: BodyRes<'a>,
    components: ComponentMap<G>,
}

#[component]
pub fn MDSycX<'a, G: Html>(cx: Scope<'a>, props: MdSycXProps<'a, G>) -> View<G> {
    events_to_view(cx, &props.body.events, props.components)
}

enum TagType<'a, G: Html> {
    Element(&'a str),
    Component(&'a TypeErasedComponent<G>),
}

fn events_to_view<'a, G: Html>(
    cx: Scope<'a>,
    events: &[Event<'a>],
    components: ComponentMap<G>,
) -> View<G> {
    // A stack of fragments. The bottom fragment is the view that is returned. Subsequent fragments
    // are those in nested elements.
    let mut fragments_stack = vec![Vec::new()];
    // Attributes that should be added when end tag is reached.
    let mut attr_stack = vec![Vec::new()];
    // Elements that should be constructed when the end tag is reached.
    let mut tag_stack = Vec::new();
    for ev in events {
        match ev {
            Event::Start(tag) => {
                fragments_stack.push(Vec::new());
                attr_stack.push(Vec::new());
                // Check if a component is registered for the tag.
                if let Some(component) = components.map.get(&tag.to_string()) {
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
                        let node = G::element_from_tag(tag);
                        // Add children to node.
                        let children = fragments_stack.pop().expect("events are not balanced");
                        insert(cx, &node, View::new_fragment(children), None, None, false);
                        // Add attributes to node.
                        let attributes = attr_stack.pop().expect("events are not balanced");
                        for (name, value) in attributes {
                            node.set_attribute(name, value);
                        }
                        fragments_stack
                            .last_mut()
                            .expect("should always have at least one fragment on stack")
                            .push(View::new_node(node));
                    }
                    TagType::Component(component) => {
                        // Collect attributes into serde_json::Value.
                        let attributes = attr_stack.pop().expect("events are not balanced");
                        let props = Value::Object(Map::from_iter(attributes.into_iter().map(
                            |(name, value)| {
                                // let value = serde_json::from_str(value)
                                //     .expect("could not deserialize value");
                                let value = Value::String(value.to_string());
                                (name.to_string(), value)
                            },
                        )));
                        // TODO: fragments are currently ignored.
                        let _children = fragments_stack.pop().expect("events are not balanced");
                        let node = component(cx, props);
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
                    .push((name, value));
            }
            Event::Text(text) => fragments_stack
                .last_mut()
                .expect("should always have at least one fragment on stack")
                .push(View::new_node(G::text_node(text))),
        }
    }

    View::new_fragment(fragments_stack[0].clone())
}
