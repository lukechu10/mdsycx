//! Sycamore bindings for rendering MD with components.

use std::collections::HashMap;
use std::rc::Rc;

use sycamore::prelude::*;
use sycamore::web::{console_warn, ViewHtmlNode, ViewNode};

use crate::{BodyRes, Event, FromMd};

type MdComponentProps = (Vec<(String, String)>, Children);

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
#[derive(Default, Clone)]
pub struct ComponentMap {
    map: HashMap<String, MdComponent>,
}

impl ComponentMap {
    /// Create a new empty [`ComponentMap`].
    pub fn new() -> Self {
        Self::default()
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

fn events_to_view(events: Vec<Event<'static>>, components: ComponentMap) -> View {
    // A stack of fragments. The bottom fragment is the view that is returned. Subsequent fragments
    // are those in nested elements.
    let mut fragments_stack: Vec<Vec<View>> = vec![Vec::new()];
    // Attributes that should be added when end tag is reached.
    let mut attr_stack: Vec<Vec<(String, String)>> = vec![Vec::new()];
    // Elements that should be constructed when the end tag is reached.
    let mut element_stack = Vec::new();
    let mut events = events.into_iter();
    while let Some(ev) = events.next() {
        match ev {
            Event::Start(tag) => {
                // Check if a component is registered for the tag.
                if let Some(component) = components.map.get(&tag.to_string()).cloned() {
                    // Render the component instead of the element.
                    //
                    // To ensure proper nesting, get all the events until the corresponding end
                    // tag. Then create a closure that recursively calls `events_to_view`.
                    let mut children_events = Vec::new();
                    let mut component_attributes = Vec::new();
                    let mut depth = 1;
                    loop {
                        let Some(ev) = events.next() else {
                            // If there are no more events and we are still in the loop, then the
                            // component is not closed.
                            console_warn!("tags are not balanced");
                            break;
                        };
                        match &ev {
                            Event::Start(_) => depth += 1,
                            Event::End => depth -= 1,
                            Event::Attr(name, value) => {
                                component_attributes.push((name.to_string(), value.to_string()))
                            }
                            _ => {}
                        }
                        // If depth is 0, we have reached the end of the component.
                        if depth == 0 {
                            break;
                        }
                        // Only push the event if it is not an attribute of the current component.
                        else if !(matches!(ev, Event::Attr(_, _)) && depth == 1) {
                            children_events.push(ev);
                        }
                    }

                    // Now call the component.
                    let components = components.clone();
                    let view = component((
                        component_attributes,
                        Children::new(move || events_to_view(children_events, components)),
                    ));
                    fragments_stack
                        .last_mut()
                        .expect("should always have at least one fragment on stack")
                        .push(view);
                } else {
                    fragments_stack.push(Vec::new());
                    attr_stack.push(Vec::new());
                    element_stack.push(tag);
                }
            }
            Event::End => {
                let tag = element_stack.pop().expect("events are not balanced");
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
