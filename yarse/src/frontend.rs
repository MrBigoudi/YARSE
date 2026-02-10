use crate::backend::{crawl_page, get_all_pages, DbPage};
use crate::Route;
use dioxus::prelude::*;

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
    let mut pages = use_signal(Vec::<DbPage>::new);
    let mut status = use_signal(|| String::from("Ready"));
    let mut test_url = use_signal(|| String::from("https://example.com"));

    let crawl_and_fetch = move |_| {
        spawn(async move {
            status.set("Crawling...".to_string());

            // Crawl the test URL
            match crawl_page(test_url()).await {
                Ok(_) => {
                    status.set("Crawl complete! Fetching from DB...".to_string());

                    // Fetch all pages from database
                    match get_all_pages().await {
                        Ok(db_pages) => {
                            // Print to console
                            for page in &db_pages {
                                println!("{:#?}", page);
                            }

                            pages.set(db_pages);
                            status.set("Success!".to_string());
                        }
                        Err(e) => {
                            status.set(format!("Error fetching pages: {}", e));
                        }
                    }
                }
                Err(e) => {
                    status.set(format!("Error crawling: {}", e));
                }
            }
        });
    };

    rsx! {
        div { id: "pageview",
            h1 { "Search Engine Test" }

            div { class: "test-controls",
                input {
                    r#type: "text",
                    value: "{test_url}",
                    oninput: move |evt| test_url.set(evt.value()),
                }
                button { onclick: crawl_and_fetch, "Crawl & Check DB" }
            }

            p { "Status: {status}" }

            div { class: "results",
                h2 { "Database Contents ({pages().len()} pages):" }

                for page in pages() {
                    div { class: "page-entry",
                        h3 { "{page.title}" }
                        p {
                            strong { "URL: " }
                            "{page.url}"
                        }
                        p {
                            strong { "Date: " }
                            "{page.date}"
                        }
                        p {
                            strong { "Meta: " }
                            "{page.meta}"
                        }
                        p {
                            strong { "Subtitles: " }
                            "{page.subtitles}"
                        }
                        p {
                            strong { "Links: " }
                            li {
                                for link in page.links {
                                    ul { "{link}" }
                                }
                            }
                        }
                        details {
                            summary { "Content ({page.content.len()} chars)" }
                            p { style: "white-space: pre-wrap; max-height: 200px; overflow-y: auto;",
                                "{page.content}"
                            }
                        }
                        hr {}
                    }
                }
            }
        }
    }
}
