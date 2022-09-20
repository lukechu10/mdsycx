use mdsycx::{parse, ComponentMap, MDSycX};
use serde::Deserialize;
use sycamore::prelude::*;

static MARKDOWN: &str = r#"
# Markdown in Sycamore

This is text is rendered with MDSycX.

```rust
fn main() {
    // A rust codeblock.
}
```

> A block quote
> ### With a heading inside.

<https://google.com>
"#;

#[derive(Prop, Deserialize)]
struct LinkProps {
    href: String,
}

#[component]
fn Link<G: Html>(cx: Scope, props: LinkProps) -> View<G> {
    let mut counter = create_signal(cx, 0);
    let increment = move |_| counter += 1;

    view! { cx,
        "Custom link to: " (props.href)
        br {}
        "Counter value: " (counter.get())
        button(on:click=increment) { "+1" }
    }
}

#[component]
fn App<G: Html>(cx: Scope) -> View<G> {
    let parsed = parse::<()>(MARKDOWN).expect("could not parse markdown");

    view! { cx,
        h1 { "MDSycX" }

        MDSycX(body=parsed.body, components=ComponentMap::new().with("a", Link))
    }
}

fn main() {
    console_error_panic_hook::set_once();

    sycamore::render(App);
}
