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
- `/logout` - logout and redirect to `/`
- `/mdguide` => `templates/mdguide.html`
- `/u` => `templates/viewuser.html` - your profile; redirect to `/login` when not logged in
- `/u/{username}` => `templates/viewuser.html` - user's profile + list of their projects; when your page, give extra options
- `/u/{username}/{project_slug}` => `templates/viewproject.html` - project page + list of logs; when your project, give extra options
- `/u/{username}/{project_slug}/{log_number}` => `templates/viewlog.html` - log page; when your log, give extra options
- `/new/project` => `templates/newproject.html` - create a new project; redirect to `/login` when not logged in
- `/new/log/{project_slug}` => `templates/newlog.html` - create a new log for the specified project; redirect to `/login` when not logged in
- `/new/media/{project_slug}` - return a json list of existing files on GET; download the file and return a json of the uploaded file on POST; new log
- `/new/media/{project_slug}/{log_number}` - return a json list of existing files on GET; download the file and return a json of the uploaded file on POST; specified log
- `/edit/log/{project_slug}/{log_number}` - give a view for editing an existing log (only today's allowed)
- `/del/project/{project_slug}` - delete the project; redirect to `/login` when not logged in
- `/del/log/{project_slug}/{log_number}` - delete the log for the specified project; redirect to `/login` when not logged in
- `/del/media/{project_slug}/new/{file_name}` - delete the given file from the newlog
- `/del/media/{project_slug}/{log_number}/{file_name}` - delete the given file from the specified log
- `/comment/{username}/{project_slug}/{log_number}` - get all comments of a log on GET; upload a comment on POST
- `/uploads/{username}/{project_slug}/{log_number}/{filename}` => `uploads/{username}/{project_slug}/{log_number}/{filename}` - uploaded files for every log go here
- `/static/*` => `static/*` - any static files that may be retrieved by the client (favicon, css/js, etc)
- `/bits/nav-user` => `templates/bits/login.html`/`nav_user.html` - get the navbar login/signup or the logout/time until logsday; bits uris are htmx helpers

## Tech Stack

- Database:
  - `SQLite`
- Web:
  - `HTMX`
  - `marked.js` live markdown preview
  - `highlight.js` highlighting code blocks
- Rust Crates:
  - `axum` for web server basics
  - `aksama` for template rendering
  - `argon2` for password hashing
  - `pulldown-cmark` for markdown to html rendering
  - `sqlx` for interacting with sqlite db
  - `infer` for scanning magic bytes of files
  - `image` for converting image formats
  - `scraper` for looking through html files for linked/embedded files
  - `tokio_cron_scheduler` for cron jobs from Rust, for now
  - `tower_sessions` for easy sessions
  - `axum_typed_multipart` for convenience, for now

## Notes

- A lot of research for this project was done with Gemini. I don't think it's a big deal, but thought I'd put it here. It's just very convenient. And I don't need the nuiance of deep-diving into the topics yet. If capstone taught me something - I shouldn't be as afraid to just do something with a moderate amount of planning. Overplanning can be overwhelming and unproductive (to me).
- `POST` responses should be either: Error message string (will be shown with htmx) or an `HX_Redirect` that redirects to the new page. Any exceptions will be noted here and in the code.
  - Exception 1: `/new/media/...` should return a json with `{error}` or `{filename, filesize, filepath}`
- Unix epoch starts on `Thu, Jan 1, 1970`. For an 8-day week, Unix epoch starts on `Mon, Jan 1, 1970`. In code, all weekdays are 0-indexed (Mon = 0, Tue = 1, etc).
- You will not be able to private a project/log. You can unlist it, but not private.

## Comprehensive .md rendering rules

- CommonMark spec
  - server side uses `pulldown_cmark` with the following options enabled
- Additional options:
  - ENABLE_TABLES - idk the syntax for tables; may not be included later, if difficult to do
  - ~strikethrough~
- Additional features:
  - `![](name.ext)` represents a multimedia embed, depending on `.ext`. For supported extensions, refer to `filestuff::media_type`

## SQLite Tables
```sql
CREATE TABLE users (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    displayname TEXT NOT NULL,
    password TEXT NOT NULL,
    week_len INTEGER NOT NULL DEFAULT 8,
    logsday_weekday INTEGER NOT NULL DEFAULT 3, -- Logsday is between Wednesday and Thursday; Monday is 0; Sunday is 6/7
    schedule_last_changed INTEGER NOT NULL, -- when the user changed their Logsday selection last time; unix timestamp
    admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_on INTEGER NOT NULL -- unix timestamp
);

CREATE TABLE projects (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    user_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT,
    created_on INTEGER NOT NULL, -- unix timestamp

    UNIQUE(user_uid, slug)
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE
);

CREATE TABLE logs (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    project_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    number INTEGER NOT NULL, -- this log's sequential number in the project
    created_on INTEGER NOT NULL, -- unix timestamp

    UNIQUE(project_uid, number)
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);

CREATE TABLE log_comments (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    log_uid INTEGER NOT NULL,
    user_uid INTEGER NOT NULL,
    text TEXT NOT NULL,
    created_on INTEGER NOT NULL, -- unix timestamp

    FOREIGN KEY (log_uid) REFERENCES logs(uid) ON DELETE CASCADE
    FOREIGN KEY (user_uid) REFERENCES users(uid)
);
```

## TODO list (no particular order)
- Make `profile` page (choose logsday day once a week, change displayname)
- Let edit project info (title, thumbnail)
- Improve comments
	- (maybe) Let delete comments
	- (maybe) Let edit comments
	- Let reply to comments
	- Add comments to user/project pages
- Let like/dislike log/project/user
- Add updates (`{last_log#}.{update#}`)
- Highlight code blocks in Rust
- Add tags to logs/projects/users
- Search logs/projects/users by name, tags
- Follow users/projects
- Make/follow groups
- Make discover page
- Add "report" button
- Refactor the uri paths to be better
- Rework the navbar
- Compress videos with FFmpeg
- Add miscellaneous pages