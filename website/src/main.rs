use mdsycx::{parse, ComponentMap, FromMd, MDSycX};
use sycamore::prelude::*;
use wasm_bindgen::prelude::*;

static MARKDOWN: &str = include_str!("../index.mdx");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=Prism, js_name=highlightAll)]
    fn highlight_all();
}

#[derive(Prop, FromMd)]
struct CounterProps<'a, G: Html> {
    initial: i32,
    children: Children<'a, G>,
}

#[component]
fn Counter<'a, G: Html>(cx: Scope<'a>, props: CounterProps<'a, G>) -> View<G> {
    let mut counter = create_signal(cx, props.initial);
    let increment = move |_| counter += 1;
    let decrement = move |_| counter -= 1;
    view! { cx,
        div(class="counter") {
            button(type="button", on:click=decrement) { "-" }
            span { (counter.get()) }
            button(type="button", on:click=increment) { "+" }
        }
    }
}

#[derive(Prop, FromMd)]
struct CodeBlockProps<'a, G: Html> {
    children: Children<'a, G>,
}

#[component]
fn CodeBlock<'a, G: Html>(cx: Scope<'a>, props: CodeBlockProps<'a, G>) -> View<G> {
    let children = props.children.call(cx);
    view! { cx,
        pre(class="codeblock") {
            (children)
        }
    }
}

#[component]
fn App<G: Html>(cx: Scope) -> View<G> {
    let parsed = parse::<()>(MARKDOWN).expect("could not parse markdown");
    log::debug!("Parsed events {:?}", parsed.body);

    let components = ComponentMap::new()
        .with("Counter", Counter)
        .with("pre", CodeBlock);

    on_mount(cx, highlight_all);

    view! { cx,
        main {
            MDSycX(body=parsed.body, components=components)
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    sycamore::render(App);
}
