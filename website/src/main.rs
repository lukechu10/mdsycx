use mdsycx::{parse, ComponentMap, FromMd, MDSycX};
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
Test
"#;

#[derive(Prop, FromMd)]
struct LinkProps<'a, G: GenericNode> {
    href: String,
    children: Children<'a, G>,
}

#[component]
fn Link<'a, G: Html>(cx: Scope<'a>, props: LinkProps<'a, G>) -> View<G> {
    let children = props.children.call(cx);
    view! { cx,
        a(class="custom-link", href=props.href) {
            (children)
        }
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
