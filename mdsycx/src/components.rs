//! Sycamore bindings for rendering MD with components.

use std::collections::HashMap;
use std::rc::Rc;

use sycamore::prelude::*;
use sycamore::utils::render::insert;

use crate::{BodyRes, Event, FromMd};

type MdComponentProps<'a, G> = (Vec<(&'a str, &'a str)>, View<G>);

/// A type-erased component that can be used from Markdown.
type MdComponent<'a, G> = Rc<dyn Fn(Scope<'a>, MdComponentProps<G>) -> View<G> + 'a>;

/// Convert a Sycamore component into a type-erased component. The props need to implement
/// [`FromMd`].
fn into_type_erased_component<'a, G: Html, F, Props>(
    f: F,
) -> impl Fn(Scope<'a>, MdComponentProps<G>) -> View<G>
where
    F: Fn(Scope<'a>, Props) -> View<G>,
    Props: FromMd<G>,
{
    move |cx, (props_serialized, children)| {
        let mut props = Props::new_prop_default();
        for (name, value) in props_serialized {
            if let Err(err) = props.set_prop(name, value) {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::warn_1(&format!("error setting prop {name}: {err}").into());
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("error setting prop {name}: {err}");
            }
        }
        props.set_children(children);
        f(cx, props)
    }
}

/// A map from component names to component functions.
pub struct ComponentMap<'a, G: Html> {
    map: HashMap<String, MdComponent<'a, G>>,
}

impl<'a, G: Html> ComponentMap<'a, G> {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    pub fn with<F, Props>(mut self, name: &'static str, f: F) -> Self
    where
        F: Fn(Scope<'a>, Props) -> View<G> + 'static,
        Props: FromMd<G> + 'a,
    {
        self.map
            .insert(name.to_string(), Rc::new(into_type_erased_component(f)));
        self
    }
}

impl<'a, G: Html> Default for ComponentMap<'a, G> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Prop)]
pub struct MdSycXProps<'a, G: Html> {
    body: BodyRes<'a>,
    #[builder(default)]
    components: ComponentMap<'a, G>,
}

/// Renders your Sycamore augmented markdown.
#[component]
pub fn MDSycX<'a, G: Html>(cx: Scope<'a>, props: MdSycXProps<'a, G>) -> View<G> {
    let events = create_ref(cx, props.body.events);
    events_to_view(cx, events, props.components)
}

enum TagType<'a, G: Html> {
    Element(&'a str),
    Component(MdComponent<'a, G>),
}

fn events_to_view<'a, G: Html>(
    cx: Scope<'a>,
    events: &'a [Event<'a>],
    components: ComponentMap<'a, G>,
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
                        let children = fragments_stack.pop().expect("events are not balanced");
                        let node = component(cx, (attributes, View::new_fragment(children)));
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

    if fragments_stack.len() != 1 {
        // TODO: emit warning
    }

    View::new_fragment(fragments_stack[0].clone())
}
