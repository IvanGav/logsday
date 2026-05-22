# Logsday (rs)

This is my version of the famous Logsday website (which is currently not publically available, which is why this project aims to be publically available).

You will be able to upload a devlog exactly once a week.

## Structure

- To run the server, `cargo run`
- Entry point for the server is `src/main.rs`
- `Askama`'s templates (`#[template(path = "index.html")]`) live in `./templates` directory
  - Note that `Askama`'s templates are not *actually* html files. They're.. templates. With `{{ ... }}` being replaced with stuff before being sent to the client.
- `Axum`'s Router uses `nest_service("/static", ServeDir::new("public"))` to map requests to `/static` to take from `./public` directory. That way, any requests to CSS or JS will take from there.
- The database is `./sqlite.db`. Needs to be created manually. Before running you might need to set an env variable `DATABASE_URL="sqlite:sqlite.db"` or `DATABASE_URL="~/logsday/sqlite.db"`

## Paths

- `/` => `templates/landing.html`- home, undecided
- `/signup` => `templates/signup.html`
- `/login` => `templates/login.html`
- `/new/project` => `templates/newproject.html` - create a new project; redirect to `/login` when not logged in
- `/new/log/{project_slug}` => `templates/newlog.html` - create a new log for the specified project; redirect to `/login` when not logged in
- `/del/project/{project_slug}` - delete the project; redirect to `/login` when not logged in
- `/del/log/{project_slug}/{log_slug}` - delete the log for the specified project; redirect to `/login` when not logged in
- `/project` => `templates/projectlist.html` - list of user's projects; redirect to `/login` when not logged in
- `/project/{project_slug}` => `templates/editproject.html` - project page with owner previleges; redirect to `/login` when not logged in
- `/project/{project_slug}/{log_slug}` => `templates/editlog.html` - log page with owner previleges; redirect to `/login` when not logged in
- `/u/{username}` => `templates/user_page.html` - look at a user's profile
- `/u/{username}/{project_slug}` => `templates/project_page.html` - get a public project page
- `/u/{username}/{project_slug}/{log_slug}` => `templates/log_page.html` - get a public project's log entry
- `/uploads/{username}/{project_slug}/{log_number}/{filename}` => `uploads/{username}/{project_slug}/{log_number}/{filename}` - uploaded files for every log go here
- `/static/*` => `static/*` - any static files that may be retrieved by the client (favicon, css/js, etc)

## Tech Stack

- Database:
  - `SQLite`
  - `sqlx` crate for Rust interface with SQLite db
- Web:
  - `HTMX`
  - `SortableJS` for specific interactions
  - `marked.js` or `markdown-it` may be used for .md rendering on the web
  - `vlitejs` may be used for the video player
    - HLS (HTTP Live Streaming) is a better streaming format; so, look into it later
- Other Crates:
  - `axum` for web server basics
  - `aksama` for template rendering
  - `tower_sessions` for easy sessions
  - `axum_typed_multipart` for convenience, for now
  - `pulldown-cmark` may be used for markdown to html rendering

## Notes

- A lot of research for this project was done with Gemini. I don't think it's a big deal, but thought I'd put it here. It's just very convenient. And I don't need the nuiance of deep-diving into the topics yet. If capstone taught me something - I shouldn't be as afraid to just do something with a moderate amount of planning. Overplanning can be overwhelming and unproductive (to me).
- `POST` responses should be either: Error message string (will be shown with htmx) or an `HX_Redirect` that redirects to the new page. Any exceptions will be noted here and in the code.
- Unix epoch starts on `Thu, Jan 1, 1970`. For an 8-day week, Unix epoch starts on `Mon, Jan 1, 1970`. In code, all weekdays are 0-indexed (Mon = 0, Tue = 1, etc).

## SQLite Tables
```sql
CREATE TABLE users (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    displayname TEXT NOT NULL,
    password TEXT NOT NULL,
    week_len INTEGER NOT NULL DEFAULT 8,
    logsday_weekday INTEGER NOT NULL DEFAULT 3, -- Logsday is between Wednesday and Thursday; Monday is 0; Sunday is 6/7
    schedule_last_changed INTEGER NOT NULL -- when the user changed their Logsday selection last time
);

CREATE TABLE projects (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    user_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT,
    thumbnail_path TEXT NOT NULL, -- technically unnecessary
    created_on INTEGER NOT NULL,
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE
);

CREATE TABLE logs (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    project_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    content_path TEXT NOT NULL, -- technically unnecessary
    thumbnail_path TEXT NOT NULL, -- technically unnecessary
    created_on INTEGER NOT NULL,
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);

CREATE INDEX idx_projects_user_slug ON projects (user_uid, slug);
```