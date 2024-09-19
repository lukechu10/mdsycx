use mdsycx::{parse, ComponentMap, FromMd, MDSycX};
use sycamore::prelude::*;
use wasm_bindgen::prelude::*;

static MARKDOWN: &str = include_str!("../index.mdx");

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=Prism, js_name=highlightAll)]
    fn highlight_all();
}

#[derive(Props, FromMd)]
struct CounterProps {
    initial: i32,
}

#[component]
fn Counter(props: CounterProps) -> View {
    let mut counter = create_signal(props.initial);
    let increment = move |_| counter += 1;
    let decrement = move |_| counter -= 1;
    view! {
        div(class="counter") {
            button(r#type="button", on:click=decrement) { "-" }
            span { (counter.get()) }
            button(r#type="button", on:click=increment) { "+" }
        }
    }
}

#[derive(Props, FromMd)]
struct CodeBlockProps {
    class: String,
    children: Children,
}

#[component]
fn CodeBlock(CodeBlockProps { class, children }: CodeBlockProps) -> View {
    let children = children.call();
    view! {
        pre(class=format!("{class} codeblock")) {
            (children)
        }
    }
}

#[component]
fn App() -> View {
    let parsed = parse::<()>(MARKDOWN).expect("could not parse markdown");

    let components = ComponentMap::new()
        .with("Counter", Counter)
        .with("pre", CodeBlock);

    on_mount(highlight_all);

    view! {
        main {
            MDSycX(body=parsed.body, components=components)
        }
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        sycamore::hydrate(App);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let template = std::fs::read_to_string("dist/.stage/index.html")
            .expect("could not read staged index.html");
        let rendered = sycamore::render_to_string(App);
        let index = template.replace("%sycamore.body%", &rendered);
        std::fs::write("dist/.stage/index.html", index)
            .expect("could not write to staged index.html");
    }
}
