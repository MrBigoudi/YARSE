use dioxus::prelude::*;
use crate::Route;


#[component]
pub fn NavBar() -> Element {
    rsx! {
        div { id: "title",
            span {}
            Link { to: Route::PageView,
                h1 { "PageView" }
            }
        }
        Outlet::<Route> {}
    }
}

#[component]
pub fn PageView() -> Element {
    rsx! {
        div { id: "pageview",
            h1 { "Hello World!" }
        }
    }
}