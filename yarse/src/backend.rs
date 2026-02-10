use anyhow::Result;
use dioxus::{
    fullstack::serde::{Deserialize, Serialize},
    prelude::*,
};

use std::collections::VecDeque;
#[cfg(feature = "server")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "server")]
static DB_FOLDER: &'static str = "databases";
#[cfg(feature = "server")]
static DB_PAGES: &'static str = "pages.db";
#[cfg(feature = "server")]
static DB_LINKS: &'static str = "links.db";
#[cfg(feature = "server")]
static TABLE_PAGES: &'static str = "pages";
#[cfg(feature = "server")]
static TABLE_LINKS: &'static str = "links";

#[cfg(feature = "server")]
static PAGES_DB: std::sync::LazyLock<Arc<Mutex<rusqlite::Connection>>> = std::sync::LazyLock::new(|| {
    std::fs::create_dir_all(DB_FOLDER).expect("Failed to create the database directory");
    let conn = rusqlite::Connection::open(std::path::Path::new(DB_FOLDER).join(DB_PAGES)).expect("Failed to open database");

    conn.execute_batch(
        &format!("CREATE TABLE IF NOT EXISTS {TABLE_PAGES} (
            url TEXT PRIMARY KEY,
            date TEXT NOT NULL,
            title TEXT NOT NULL,
            meta TEXT NOT NULL,
            content TEXT NOT NULL,
            subtitles TEXT,
            links TEXT
        );"),
    )
    .expect(&format!("Failed to create the {TABLE_PAGES} table"));

    Arc::new(Mutex::new(conn))
});

#[cfg(feature = "server")]
static LINKS_DB: std::sync::LazyLock<Arc<Mutex<rusqlite::Connection>>> = std::sync::LazyLock::new(|| {
    std::fs::create_dir_all(DB_FOLDER).expect("Failed to create the database directory");
    let conn = rusqlite::Connection::open(std::path::Path::new(DB_FOLDER).join(DB_LINKS)).expect("Failed to open database");

    conn.execute_batch(
        &format!("CREATE TABLE IF NOT EXISTS {TABLE_LINKS} (
            url TEXT PRIMARY KEY,
            date TEXT NOT NULL
        );"),
    )
    .expect(&format!("Failed to create the {TABLE_LINKS} table"));

    Arc::new(Mutex::new(conn))
});




#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPage {
    pub url: String,
    pub date: String,
    pub title: String,
    pub meta: String,
    pub content: String,
    pub subtitles: String,
    pub links: Vec<String>,
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

    let links = extract_links(html, url);

    DbPage {
        url: url.to_owned(),
        date: date.to_string(),
        title,
        meta,
        content,
        subtitles,
        links,
    }
}

#[cfg(feature = "server")]
pub fn extract_links(html: &str, base_url: &str) -> Vec<String> {
    use scraper::{Html, Selector};
    use url::Url;
    
    let document = Html::parse_document(html);
    let link_selector = Selector::parse("a[href]").unwrap();
    
    let base = Url::parse(base_url).unwrap();
    let mut links = Vec::new();
    
    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            if let Ok(absolute_url) = base.join(href) {
                if absolute_url.scheme() == "http" || absolute_url.scheme() == "https" {
                    links.push(absolute_url.to_string());
                }
            }
        }
    }
    
    links
}

#[server]
pub async fn fetch_page(url: String) -> Result<DbPage> {
    use reqwest;
    // Fetch the HTML
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; MySearchBot/1.0)")
        .build()?;

    let response = client.get(&url).send().await?;
    let html = response.text().await?;

    let date = chrono::Utc::now().naive_utc();
    let db_page = parse_page(&url, &html, &date);
    Ok(db_page)
}

#[server]
pub async fn crawl_page(url: String) -> Result<()> {
    println!("Crawling page {url}");
    let db_page = fetch_page(url.clone()).await?;

    // Store the page in database
    let db = PAGES_DB.lock().unwrap();
    db.execute(
        &format!("INSERT OR REPLACE INTO {TABLE_PAGES} (url, date, title, meta, content, subtitles, links) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"), 
        rusqlite::params![
            db_page.url,
            db_page.date,
            db_page.title,
            db_page.meta,
            db_page.content,
            db_page.subtitles,
            db_page.links.join(";"),
        ]
    )?;

    // Store the links in database
    let db = LINKS_DB.lock().unwrap();
    for link in db_page.links {
        db.execute(
            &format!("INSERT OR REPLACE INTO {TABLE_LINKS} (url, date) VALUES (?1, ?2)"), 
            rusqlite::params![
                link,
                db_page.date,
            ]
        )?;
    }

    Ok(())
}

#[server]
pub async fn get_all_pages() -> Result<Vec<DbPage>> {
    let db = PAGES_DB.lock().unwrap();
    let mut stmt = db.prepare(
        &format!("SELECT url, date, title, meta, content, subtitles, links FROM {TABLE_PAGES}")
    )?;

    let pages = stmt
        .query_map([], |row| {
            let links: String = row.get(6)?;
            Ok(DbPage {
                url: row.get(0)?,
                date: row.get(1)?,
                title: row.get(2)?,
                meta: row.get(3)?,
                content: row.get(4)?,
                subtitles: row.get(5)?,
                links: links.split(';').map(|s| s.to_string()).collect(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(pages)
}

#[cfg(feature = "server")]
pub fn update_urls_queue(urls_queue: &mut VecDeque<String>) -> Result<()> {
    let links_db = LINKS_DB.lock().unwrap();
    let mut stmt = links_db.prepare(&format!("SELECT * FROM {TABLE_LINKS}")).unwrap();
    let mut rows = stmt.query([]).unwrap();
    let mut counter = 0usize;
    let nb_urls_to_add = urls_queue.capacity() - urls_queue.len();
    'inner_loop: while let Some(row) = rows.next().unwrap() {
        if counter >= nb_urls_to_add {
            break 'inner_loop;
        }
        let url: String = row.get(0).unwrap();
        
        // If the url is being crawled or if it was already crawled, do not add it to the queue
        if urls_queue.contains(&url){
            continue 'inner_loop;
        }
        let pages_db = PAGES_DB.lock().unwrap();
        let exists = pages_db
            .query_row(
                &format!("SELECT url FROM {TABLE_PAGES} WHERE url = ?1"),
                rusqlite::params![url],
                |_| Ok(()),
            ).is_ok()
        ;
        
        if !exists {
            counter += 1;
            urls_queue.push_back(url);
        }
    }
    Ok(())
}

#[server]
pub async fn run_crawlers(max_loops: Option<usize>) -> Result<()> {
    let min_urls = 32usize;
    let max_urls = 2usize << 15; // ~32k
    let mut urls_queue: VecDeque<String> = VecDeque::with_capacity(max_urls);
    
    // Inifinite loop to refill the queue of urls
    let mut cpt = 0usize;
    'main_loop: loop {
        // If not enough urls in the queue, fill it up with urls from the DB
        if urls_queue.len() < min_urls {
            update_urls_queue(&mut urls_queue)?;
        }
        if urls_queue.is_empty(){
            println!("Can't find more urls");
            break 'main_loop;
        }
        if let Some(max_loops) = max_loops {
            cpt += 1;
            if cpt > max_loops {
                break 'main_loop;
            };
        }

        // TODO: run crawlers in parallel
        if let Some(url) = urls_queue.pop_front(){
            crawl_page(url).await?;
        }
    }

    Ok(())
}

















//////////////////////////////////////////////////////////
///////////////////     tests      ///////////////////////
//////////////////////////////////////////////////////////
#[cfg(test)]
mod tests {
    use super::*;

    // #[tokio::test]
    // async fn test_crawl() {
    //     let url = "https://doc.rust-lang.org/stable/book/";

    //     match crawl_page(url.to_string()).await {
    //         Ok(_) => {
    //             println!("Crawling of {:?} successful!", url);

    //             // Query the page database
    //             let db = PAGES_DB.lock().unwrap();
    //             let mut stmt = db.prepare(&format!("SELECT * FROM {TABLE_PAGES}")).unwrap();
    //             let mut rows = stmt.query([]).unwrap();
    //             while let Some(row) = rows.next().unwrap() {
    //                 let url: String = row.get(0).unwrap();
    //                 let date: String = row.get(1).unwrap();
    //                 let title: String = row.get(2).unwrap();
    //                 let meta: String = row.get(3).unwrap();
    //                 let content: String = row.get(4).unwrap();
    //                 let subtitles: String = row.get(5).unwrap();
    //                 let links: String = row.get(6).unwrap();
    //                 println!("\n=== PAGE ===");
    //                 println!("URL: {}", url);
    //                 println!("Date: {}", date);
    //                 println!("Title: {}", title);
    //                 println!("Meta: {}", meta);
    //                 println!("Subtitles: {}", subtitles);
    //                 println!("Links: {}", links);
    //                 println!(
    //                     "Content (first 200 chars): {}",
    //                     &content.chars().take(200).collect::<String>()
    //                 );
    //             }

    //             // Query the links database
    //             let db = LINKS_DB.lock().unwrap();
    //             let mut stmt = db.prepare(&format!("SELECT * FROM {TABLE_LINKS}")).unwrap();
    //             let mut rows = stmt.query([]).unwrap();
    //             while let Some(row) = rows.next().unwrap() {
    //                 let url: String = row.get(0).unwrap();
    //                 let date: String = row.get(1).unwrap();
    //                 println!("\n=== LINK ===");
    //                 println!("URL: {}", url);
    //                 println!("Date: {}", date);
    //             }
    //         }
    //         Err(e) => {
    //             eprintln!("Error: {}", e);
    //         }
    //     }
    // }

    #[tokio::test]
    async fn test_crawlers() {
        let url = "https://example.com";
        let date = chrono::Utc::now().naive_utc().to_string();

        // Store the link in database
        LINKS_DB.lock().unwrap().execute(
            &format!("INSERT OR REPLACE INTO {TABLE_LINKS} (url, date) VALUES (?1, ?2)"), 
            rusqlite::params![url, date]
        ).unwrap();

        // match run_crawlers(Some(5)).await {
        match run_crawlers(None).await {
            Ok(_) => {
                println!("Done running all crawlers!");

                // Query the page database
                let db = PAGES_DB.lock().unwrap();
                let mut stmt = db.prepare(&format!("SELECT * FROM {TABLE_PAGES}")).unwrap();
                let mut rows = stmt.query([]).unwrap();
                while let Some(row) = rows.next().unwrap() {
                    let url: String = row.get(0).unwrap();
                    let date: String = row.get(1).unwrap();
                    let title: String = row.get(2).unwrap();
                    let meta: String = row.get(3).unwrap();
                    let content: String = row.get(4).unwrap();
                    let subtitles: String = row.get(5).unwrap();
                    let links: String = row.get(6).unwrap();
                    println!("\n=== PAGE ===");
                    println!("URL: {}", url);
                    println!("Date: {}", date);
                    println!("Title: {}", title);
                    println!("Meta: {}", meta);
                    println!("Subtitles: {}", subtitles);
                    println!("Links: {}", links);
                    println!(
                        "Content (first 200 chars): {}",
                        &content.chars().take(200).collect::<String>()
                    );
                }

                // Query the links database
                let db = LINKS_DB.lock().unwrap();
                let mut stmt = db.prepare(&format!("SELECT * FROM {TABLE_LINKS}")).unwrap();
                let mut rows = stmt.query([]).unwrap();
                while let Some(row) = rows.next().unwrap() {
                    let url: String = row.get(0).unwrap();
                    let date: String = row.get(1).unwrap();
                    println!("\n=== LINK ===");
                    println!("URL: {}", url);
                    println!("Date: {}", date);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}
