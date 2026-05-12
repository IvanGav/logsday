# Logsday (rs)

This is my version of the famous Logsday website (which is currently not publically available, which is why this project aims to be publically available).

You will be able to upload a devlog exactly once a week.

## Structure

- To run the server, `cargo run`
- Entry point for the server is `src/main.rs`
- `Askama`'s templates (`#[template(path = "index.html")]`) live in `./templates` directory
  - Note that `Askama`'s templates are not *actually* html files. They're.. templates. With `{{ ... }}` being replaced with stuff before being sent to the client.
- `Axum`'s Router uses `nest_service("/static", ServeDir::new("public"))` to map requests to `/static` to take from `./public` directory. That way, any requests to CSS or JS will take from there.
- The database is `./sqlite.db`. Before running you might need to set an env variable `DATABASE_URL="sqlite:sqlite.db"` or `DATABASE_URL="~/logsday/sqlite.db"`

## Tech (+ yapping)

- Ok I'm pretty sold on SQLite, Miles didn't choose it for nothing
- SortableJS looks absolutely incredible for smooth drag and drop animations
- I think HTMX is pretty neat, but something like Svelte or literally raw HTML/JS might be just fine.. I'll experiment with HTMLX first
- Obviously, Axum for the backend. Ridiculously simple and robust.
  - Askama sounds pretty neat - Jinja-like templates but with rust.
  - [SQLx](https://docs.rs/sqlx/latest/sqlx/) is a pretty neat looking library to work with SQL from Rust
- For video play, `<video>` html tag can be used; libraries and wrappers exist (vlitejs, etc)
- Also, HLS (HTTP Live Streaming) is a better streaming format; so, look into it later
- For markdown probably `pulldown-cmark`, looks quite nice. As for JS side, probably either `marked.js` or `markdown-it`.

## Notes

- For sessions I might use `Set-Cookie` http header and then store the session cookie. Btw, i have no clue how actual websites work, this is my first time researching how to make a "real" website.
- Also, a lot of this research so far was done with Gemini. I don't think it's a big deal, but thought I'd put it here. It's just very convenient. And I don't need the nuiance of deep-diving into the topics yet. If capstone taught me something - I shouldn't be as afraid to just do something with a moderate amount of planning. Overplanning can be overwhelming and unproductive (to me).

## Links

- [SortableKS](https://sortablejs.github.io/Sortable/)
- [htmx](https://htmx.org/) ([docs](https://htmx.org/docs/))
- [Askama docs](https://askama.rs/en/stable/template_syntax.html#template-inheritance), [Askama overview](https://blog.guillaume-gomez.fr/articles/2025-03-19+Askama+and+Rinja+merge)
- [md](https://crates.io/crates/pulldown-cmark)

## SQLite Tables
```sql
CREATE TABLE users (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    displayname TEXT NOT NULL,
    password TEXT NOT NULL
);

CREATE TABLE projects (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    user_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT,
    thumbnail_path TEXT NOT NULL,
    created_on DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE
);

CREATE TABLE logs (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    project_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    content_path TEXT NOT NULL,
    thumbnail_path TEXT NOT NULL,
    created_on DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);
```