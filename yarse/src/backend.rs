use anyhow::Result;
use dioxus::{
    fullstack::serde::{Deserialize, Serialize},
    prelude::*,
};

#[cfg(feature = "server")]
use std::sync::Mutex;

#[cfg(feature = "server")]
static WEB_DB: std::sync::LazyLock<Mutex<rusqlite::Connection>> = std::sync::LazyLock::new(|| {
    std::fs::create_dir_all("webdb").expect("Failed to create webdb directory");
    let conn = rusqlite::Connection::open("webdb/web.db").expect("Failed to open database");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS pages (
            url TEXT PRIMARY KEY,
            date TEXT NOT NULL,
            title TEXT NOT NULL,
            meta TEXT NOT NULL,
            content TEXT NOT NULL,
            subtitles TEXT
        );",
    )
    .expect("Failed to create table");

    Mutex::new(conn)
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPage {
    pub url: String,
    pub date: String,
    pub title: String,
    pub meta: String,
    pub content: String,
    pub subtitles: String,
}

#[cfg(feature = "server")]
pub fn parse_page(url: &str, html: &str, date: &chrono::NaiveDateTime) -> DbPage {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);

    // Extract title
    let title_selector = Selector::parse("title").unwrap();
    let title = if let Some(title_elem) = document.select(&title_selector).next() {
        title_elem.inner_html()
    } else {
        String::new()
    };

    // Extract meta information
    let mut meta_parts = Vec::new();
    if let Ok(meta_desc_sel) =
        Selector::parse("meta[name='description'], meta[property='og:description']")
    {
        if let Some(elem) = document.select(&meta_desc_sel).next() {
            if let Some(content) = elem.value().attr("content") {
                meta_parts.push(content.to_string());
            }
        }
    }
    let meta = meta_parts.join(";");

    // Extract content from body
    let content_selector = Selector::parse("body").unwrap();
    let content = if let Some(body) = document.select(&content_selector).next() {
        body.text().collect::<Vec<_>>().join(";")
    } else {
        String::new()
    };

    // Extract subtitles
    let subtitles_selector = Selector::parse("h1, h2, h3, h4").unwrap();
    let subtitles: Vec<String> = document
        .select(&subtitles_selector)
        .map(|elem| elem.inner_html())
        .collect();
    let subtitles = subtitles.join(";");

    DbPage {
        url: url.to_owned(),
        date: date.to_string(),
        title,
        meta,
        content,
        subtitles,
    }
}

#[server]
pub async fn fetch_page(url: String) -> Result<(DbPage, Vec<String>)> {
    use reqwest;
    println!("Fetching: {}", url);
    // Fetch the HTML
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; MySearchBot/1.0)")
        .build()?;

    let response = client.get(&url).send().await?;
    let html = response.text().await?;

    println!("Fetched {} bytes", html.len());
    let date = chrono::Utc::now().naive_utc();
    // Parse the page
    let db_page = parse_page(&url, &html, &date);
    // // Extract links
    // let links = extract_links(&html, &url);
    let links = vec![];
    Ok((db_page, links))
}

#[server]
pub async fn crawl_page(url: String) -> Result<()> {
    let (db_page, _links) = fetch_page(url.clone()).await?;

    // Store the page in database
    let db = WEB_DB.lock().unwrap();
    db.execute(
        "INSERT OR REPLACE INTO pages (url, date, title, meta, content, subtitles) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", 
        rusqlite::params![
            db_page.url,
            db_page.date,
            db_page.title,
            db_page.meta,
            db_page.content,
            db_page.subtitles
        ]
    )?;

    // println!("Stored page: {}", db_page.url);
    // println!("Title: {}", db_page.title);
    // println!("Found links: {}", links.len());

    // TODO: Store links in a separate table for crawling queue

    Ok(())
}

#[server]
pub async fn get_all_pages() -> Result<Vec<DbPage>> {
    let db = WEB_DB.lock().unwrap();
    let mut stmt = db.prepare("SELECT url, date, title, meta, content, subtitles FROM pages")?;

    let pages = stmt
        .query_map([], |row| {
            Ok(DbPage {
                url: row.get(0)?,
                date: row.get(1)?,
                title: row.get(2)?,
                meta: row.get(3)?,
                content: row.get(4)?,
                subtitles: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_crawl() {
        let url = "https://doc.rust-lang.org/stable/book/";

        match crawl_page(url.to_string()).await {
            Ok(_) => {
                println!("Crawling of {:?} successful!", url);

                // Query the database
                let db = WEB_DB.lock().unwrap();

                let mut stmt = db.prepare("SELECT * FROM pages").unwrap();
                let mut rows = stmt.query([]).unwrap();

                while let Some(row) = rows.next().unwrap() {
                    let url: String = row.get(0).unwrap();
                    let date: String = row.get(1).unwrap();
                    let title: String = row.get(2).unwrap();
                    let meta: String = row.get(3).unwrap();
                    let content: String = row.get(4).unwrap();
                    let subtitles: String = row.get(5).unwrap();

                    println!("\n=== PAGE ===");
                    println!("URL: {}", url);
                    println!("Date: {}", date);
                    println!("Title: {}", title);
                    println!("Meta: {}", meta);
                    println!("Subtitles: {}", subtitles);
                    println!(
                        "Content (first 200 chars): {}",
                        &content.chars().take(200).collect::<String>()
                    );
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}
