use anyhow::Result;
use dioxus::prelude::*;

#[cfg(feature = "server")]
thread_local! {
    static DB: std::sync::LazyLock<rusqlite::Connection> = std::sync::LazyLock::new(|| {
        std::fs::create_dir("webdb").unwrap();
        let conn = rusqlite::Connection::open("webdb/web.db").expect("Failed to open database");

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pages (
                url INTEGER PRIMARY KEY,
                content TEXT NOT NULL
                h1s TEXT
                h2s TEXT
                h3s TEXT
                h4s TEXT
                meta TEXT
            );",
        )
        .unwrap();

        conn
    });
}

#[get("/api/pages")]
pub async fn list_pages() -> Result<Vec<(usize, String)>> {
    DB.with(|db| {
        Ok(db
            .prepare("SELECT id, url FROM pages ORDER BY id DESC LIMIT 10")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<(usize, String)>, rusqlite::Error>>()?)
    })
}

#[delete("/api/pages/{id}")]
pub async fn remove_page(id: usize) -> Result<()> {
    DB.with(|db| db.execute("DELETE FROM pages WHERE id = ?1", [id]))?;
    Ok(())
}

#[post("/api/pages")]
pub async fn save_page(image: String) -> Result<()> {
    DB.with(|db| db.execute("INSERT INTO pages (url) VALUES (?1)", [&image]))?;
    Ok(())
}