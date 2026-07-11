use std::{fs, io::Cursor, path::Path, collections::HashSet};
use image::ImageFormat;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use tokio::process::Command;

const INVALID_FILENAME_CHARACTERS: [char; 10] = ['*', '"', '/', '\\', '<', '>', ':', '|', '?', '\0'];

#[derive(PartialEq)]
pub enum MediaType {
    Unsupported, Image, Video, Audio,
}

/// Given a *valid* filename, return its media type by extension
pub fn media_type(filename: &str) -> MediaType {
    let split = filename.rsplit_once('.');
    if let None = split { return MediaType::Unsupported; } // if no extension
    let split = split.unwrap();
    if split.0 == "" { return MediaType::Unsupported; } // no name -> `.ext` is the name
    match split.1.to_lowercase().as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "ico" /* | "svg" */ => MediaType::Image,
        "mp4" | "mpeg" | "webm" | "ogv" | "ogg" /* | "mov" */ => MediaType::Video,
        "mp3" | "wav" | "oga" | "weba" => MediaType::Audio,
        _ => MediaType::Unsupported,
    }
}

/// Given a mime type, return its kind
/// [list](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/MIME_types/Common_types)
pub fn mime_media_type(mime_type: &str) -> MediaType {
    let mime_type = mime_type.split_once(';').unwrap_or((mime_type, "")).0; // get rid of parameters: `type/subtype;parameter=value`
    match mime_type.to_lowercase().as_str() {
        "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/avif" | "image/x-icon" /* | "image/svg+xml" */ => MediaType::Image,
        "video/mp4" | "video/mpeg" | "video/webm" | "video/ogg" /* | "video/quicktime" */ => MediaType::Video,
        "audio/mpeg" | "audio/wav" | "audio/ogg" | "audio/webm" => MediaType::Audio,
        u => { println!("{u}"); MediaType::Unsupported },
    }
}

/// Checks for name size limits (255), being '.' or '..' and for any invalid characters (INVALID_FILENAME_CHARACTERS, which includes Windows invalid filename characters)
/// **Does not check for Windows reserved filenames!!**
pub fn filename_valid(filename: &str) -> bool {
    if filename.len() > 255 { return false; }
    if filename == "." || filename == ".." || filename == "" { return false; }
    if filename.contains(&INVALID_FILENAME_CHARACTERS) { return false; }
    return true;
}

/// Make sure that extensions are: lowercase, unique per file type
pub fn normalize_extension(filename: &str) -> String {
    let split = filename.rsplit_once('.').unwrap();
    let mut ext = split.1.to_ascii_lowercase();
    match ext.as_str() {
        "jpeg" => { ext = "jpg".into(); }
        _ => { }
    }
    return split.0.to_string() + "." + &ext;
}

pub fn convert_to_webp(raw_bytes: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let img = image::load_from_memory(raw_bytes)?;
    let img = img.thumbnail(400, 400);
    let mut webp_buffer = Vec::new();
    img.write_to(&mut Cursor::new(&mut webp_buffer), ImageFormat::WebP)?;
    return Ok(webp_buffer);
}

// disallow embedding html
pub fn render_markdown_to_html(markdown_input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown_input, options);

    let parser = parser.map(|event| match event {
        pulldown_cmark::Event::Html(text) | pulldown_cmark::Event::InlineHtml(text) => {
            pulldown_cmark::Event::Text(text) // convert html/inline html into just text - no html embedding at all allowed
        }
        other => other,
    });

    let mut is_video = false;

    let parser = parser.filter_map(|event| {
        match &event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                if dest_url.ends_with(".mp4") {
                    is_video = true;
                    let video_url = dest_url.to_string();
                    let mime_type = "video/mp4";
                    let video_html = format!(
                        r#"<video controls><source src="{}" type="{}"></source>"#,
                        video_url, mime_type
                    );
                    Some(Event::Html(video_html.into()))
                } else {
                    Some(event)
                }
            }

            Event::End(TagEnd::Image { .. }) => {
                if is_video {
                    is_video = false;
                    Some(Event::Html("</video>".into()))
                } else {
                    Some(event)
                }
            }

            // Event::Text(_) => { if is_video { None } else { Some(event) } }
            _ => Some(event),
        }
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    return html_output;
}

pub async fn get_directory_size_bytes<P: AsRef<Path>>(dir_path: P) -> std::io::Result<u64> {
    let mut total_size = 0;
    let mut entries = fs::read_dir(dir_path)?;
    while let Some(entry) = entries.next() {
        let metadata = entry?.metadata()?;
        if metadata.is_file() {
            total_size += metadata.len();
        }
    }
    return Ok(total_size);
}

pub async fn verify_magic_bytes_match_extension(filename: &str, bytes: &[u8]) -> bool {
    let kind = match infer::get(&bytes) { Some(k) => k, None => { return false; }};
    return mime_media_type(kind.mime_type()) == media_type(filename);
}

pub fn log_dir_exists(username: &str, project_slug: &str, log_num: i64) -> bool {
    return Path::new(&format!("uploads/users/{}/{}/{}", username, project_slug, log_num)).exists();
}

/// Count the size of the media files linked 
pub fn count_log_directory_size<P: AsRef<Path>>(dir_path: P, log_html_content: &str) -> std::io::Result<u64> {
    let dir = dir_path.as_ref();
    let document = scraper::Html::parse_document(&log_html_content);
    let mut linked_files = HashSet::new();
    // Look for tags that use 'src' attributes (img, video, audio, source)
    let src_selector = scraper::Selector::parse("[src]").unwrap();
    for element in document.select(&src_selector) {
        if let Some(src_val) = element.value().attr("src") {
            if let Some(filename) = Path::new(src_val).file_name() {
                linked_files.insert(filename.to_string_lossy().into_owned()); // TODO why `to_string_lossy`? why not `to_str`? because it may fail? but it really shouldn't
            }
        }
    }
    let mut all_files = match fs::read_dir(dir) { Ok(f) => f, Err(_) => { return Ok(0); }};
    let mut total_size = 0;
    while let Some(file) = all_files.next() {
        let file_path = file?.path();
        let file_name = file_path.file_name().unwrap().to_string_lossy().into_owned(); // TODO again the same thing; also it's safe to unwrap because we're already just reading the existing dir
        if !linked_files.contains(&file_name) {
            total_size += Path::new(&file_path).metadata()?.len();
        }
    }
    return Ok(total_size);
}

/// Clean up the log directory - only keep the media files linked in index.md/index.html
pub fn cleanup_log_directory<P: AsRef<Path>>(dir_path: P) -> std::io::Result<()> {
    let dir = dir_path.as_ref();
    let index_path = dir.join("index.html");
    // No index.html = log wasn't uploaded in the end; just delete the entire dir
    if !index_path.exists() {
        fs::remove_dir_all(dir)?;
        return Ok(());
    }
    let html_content = fs::read_to_string(&index_path)?;
    let document = scraper::Html::parse_document(&html_content);
    let mut linked_files = HashSet::new();
    // Look for tags that use 'src' attributes (img, video, audio, source)
    let src_selector = scraper::Selector::parse("[src]").unwrap();
    for element in document.select(&src_selector) {
        if let Some(src_val) = element.value().attr("src") {
            if let Some(filename) = Path::new(src_val).file_name() {
                linked_files.insert(filename.to_string_lossy().into_owned()); // TODO why `to_string_lossy`?
            }
        }
    }
    let mut all_files = fs::read_dir(dir)?;
    while let Some(file) = all_files.next() {
        let file_path = file?.path();
        let file_name = file_path.file_name().unwrap().to_string_lossy().into_owned(); // TODO again the same thing; also it's safe to unwrap because we're already just reading the existing dir
        if file_name == "index.html" || file_name == "index.md" { continue; }
        if !linked_files.contains(&file_name) {
            fs::remove_file(file_path)?;
        }
    }
    Ok(())
}

pub fn cleanup_all_log_directories() -> std::io::Result<()> {
    let mut users = fs::read_dir("uploads/users")?;
    while let Some(userdir) = users.next() {
        let userdir = userdir?;
        if !userdir.metadata()?.is_dir() { continue; }
        let mut projects = fs::read_dir(userdir.path())?;
        while let Some(projectdir) = projects.next() {
            let projectdir = projectdir?;
            if !projectdir.metadata()?.is_dir() { continue; }
            let mut logs = fs::read_dir(projectdir.path())?;
            while let Some(logdir) = logs.next() {
                let logdir = logdir?;
                if !logdir.metadata()?.is_dir() { continue; }
                cleanup_log_directory(logdir.path())?;
            }
        }
    }
    Ok(())
}

pub async fn compress_video(input_path: &str, output_path: &str) -> std::io::Result<()> {
    // maybe try using: nice -n 19 ffmpeg -threads 1 -i input.mov -vf "scale='min(1920,iw)':-2" -c:v libx264 -preset veryfast -crf 24 -x264opts rc-lookahead=15 -c:a aac -b:a 128k output.mp4
    let status = Command::new("ffmpeg")
        .args([
            "-i", input_path,
            "-vcodec", "libx264", // libsvtav1 for av1
            "-crf", "23", // 51 = bad; 27 for av1 is good
            "-acodec", "aac",
            "-b:a", "128k", // Audio bitrate
            "-y", // Overwrite output file if it exists
            output_path,
        ])
        .status()
        .await?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::Other, "FFmpeg execution failed"))
    }
}