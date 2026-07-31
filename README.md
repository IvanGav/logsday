# Logsday (rs)

A humble recreation of the famous Logsday website. The original website was abandoned by its creator and is no longer developed. That's why this project aims to not be abandoned and be in development.

You can upload a log once a day. You can see other people's logs. You can invite your friends to post their logs. You can even like other people's logs. Furthermore, you can look at the `TODO` list down below and contribute some of your very own bad code!

## Structure

- To run the server, `cargo run` or `cargo run --release`
  - It should be available on `http://localhost:3000/`
- `Askama` templates (`#[template(path = "index.html")]`) live in `./templates` directory
  - Note that `Askama` templates are not *actually* html files. They're.. templates. With `{{ ... }}` being replaced with stuff before being sent to the client.
- The database is `./sqlite.db`. Needs to be created manually. You will also have to create all tables manually (listed in `SQLite Tables` section).
- You have to have `nice` and `ffmpeg` cmd utilities instealled.

## Tech Stack

- Server Side:
  - `SQLite`
  - `FFmpeg`
- Web:
  - `HTMX`
  - `marked.js` live markdown preview
  - `highlight.js` highlighting code blocks
- Rust Crates:
  - `axum` for web server basics
  - `askama` for template rendering
  - `argon2` for password hashing
  - `pulldown-cmark` for markdown to html rendering
  - `sqlx` for interacting with sqlite db
  - `infer` for scanning magic bytes of files
  - `image` for converting image formats
  - `webp-animation` for specifically converting gif to webp (wrapper around `libwebp`)
  - `scraper` for looking through html files for linked/embedded files
  - `tokio_cron_scheduler` for cron jobs from Rust, for now
  - `tower_sessions` for easy sessions
  - `axum_typed_multipart` for convenience, for now
  - `tower_governor` for traffic control; currently unused

## Other

- Unix epoch starts on `Thu, Jan 1, 1970`. For an 8-day week, Unix epoch starts on `Mon, Jan 1, 1970`. In code, all weekdays are 0-indexed (Mon = 0, Tue = 1, etc).
- You will not be able to private a project/log. You will be able to unlist it, but not private.
- A bunch of research for this project was done with Gemini.

## Markdown rendering rules

- CommonMark spec
  - Server side uses `pulldown_cmark` with the following options enabled
- Additional options
  - ~strikethrough~
- Additional features
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

    UNIQUE(user_uid, slug),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE
);

CREATE TABLE logs (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    project_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    number INTEGER NOT NULL, -- this log's sequential number in the project
    created_on INTEGER NOT NULL, -- unix timestamp

    UNIQUE(project_uid, number),
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);

CREATE TABLE log_comments (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    log_uid INTEGER NOT NULL,
    user_uid INTEGER NOT NULL,
    text TEXT NOT NULL,
    created_on INTEGER NOT NULL, -- unix timestamp

    FOREIGN KEY (log_uid) REFERENCES logs(uid) ON DELETE CASCADE,
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE
);

CREATE TABLE log_likes (
    user_uid INTEGER NOT NULL,
    log_uid INTEGER NOT NULL,
    is_like BOOLEAN NOT NULL, -- like or dislike
    PRIMARY KEY (user_uid, log_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (log_uid) REFERENCES logs(uid) ON DELETE CASCADE
);

CREATE TABLE project_likes (
    user_uid INTEGER NOT NULL,
    project_uid INTEGER NOT NULL,
    is_like BOOLEAN NOT NULL, -- like or dislike
    PRIMARY KEY (user_uid, project_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);

CREATE TABLE user_likes (
    user_uid INTEGER NOT NULL,
    user_profile_uid INTEGER NOT NULL,
    is_like BOOLEAN NOT NULL, -- like or dislike
    PRIMARY KEY (user_uid, user_profile_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (user_profile_uid) REFERENCES users(uid) ON DELETE CASCADE
);

CREATE TABLE user_follows (
    user_uid INTEGER NOT NULL,
    user_profile_uid INTEGER NOT NULL,
    PRIMARY KEY (user_uid, user_profile_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (user_profile_uid) REFERENCES users(uid) ON DELETE CASCADE
);

CREATE TABLE project_follows (
    user_uid INTEGER NOT NULL,
    project_uid INTEGER NOT NULL,
    PRIMARY KEY (user_uid, project_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);
```

## TODO list (no particular order)
- Let edit project thumbnail
- Improve comments
	- (maybe) Let delete comments
	- (maybe) Let edit comments
	- Let reply to comments
	- Add comments to user/project pages
- Add updates (`{last_log#}.{update#}`)
- Highlight code blocks in Rust
- Add tags to logs/projects/users
- Search logs/projects/users by name, tags
- Follow users/projects
- Make/follow groups
- Make discover page
- Add "report" button
- Add support for mov video files (apple format, not native to browsers, probably convert to mp4)
- Inbox
- Auto create sqlite tables if they don't yet exist
- Make phone layout compatible
- Allow to unlist projects
- Fix Bugs:
  - When modifying text in markdown editor, if text is long and md side scrolled down, it will jump up.