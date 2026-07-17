use std::{collections::HashSet, fs, io::Cursor, path::Path, time::SystemTime};
use image::{AnimationDecoder, DynamicImage, Frame, ImageFormat, ImageResult, codecs::gif::GifDecoder, imageops::FilterType};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use tokio::process::Command;
use webp_animation::{WebPData, prelude::Encoder};

use crate::AppState;

const INVALID_FILENAME_CHARACTERS: [char; 10] = ['*', '"', '/', '\\', '<', '>', ':', '|', '?', '\0'];

#[derive(PartialEq)]
pub enum MediaType {
    Unsupported, Image, Video, Audio,
}

pub fn get_extension(filename: &str) -> Option<&str> {
    let split = filename.rsplit_once('.');
    if let None = split { return None; } // if no extension
    let split = split.unwrap();
    if split.0 == "" { return None; } // no name -> `.ext` is the name
    return Some(split.1);
}

/// Given a *valid* filename, return its media type by extension
pub fn media_type(filename: &str) -> MediaType {
    let ext = get_extension(filename);
    if let None = ext { return MediaType::Unsupported; }
    let ext = ext.unwrap();
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" | "ico" /* | "svg" */ => MediaType::Image,
        "mp4" | "webm" /* | "mov" */ => MediaType::Video,
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
        "video/mp4" | "video/webm" /* | "video/quicktime" */ => MediaType::Video,
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

pub fn convert_to_webp(raw_bytes: &[u8]) -> Option<Vec<u8>> {
    let format = image::guess_format(raw_bytes).ok()?;
    if format == ImageFormat::WebP {
        return Some(raw_bytes.to_owned());
    } else if format == ImageFormat::Gif {
        // Written by AI (and edited by me, but not significantly; I still don't know if there's an easier way to do this)
        let decoder = GifDecoder::new(Cursor::new(raw_bytes)).ok()?;
        let frames = decoder.into_frames().collect::<Result<Vec<_>, _>>().ok()?;
        if frames.is_empty() { return None; }
        let first_frame_img = DynamicImage::ImageRgba8(frames[0].buffer().clone());
        let thumb = first_frame_img.thumbnail(400, 400);
        let mut encoder = Encoder::new((thumb.width(), thumb.height())).ok()?;
        let mut current_timestamp_ms = 0;
        for frame in frames {
            let (delay_ms_numer, delay_ms_denom) = frame.delay().numer_denom_ms();
            let delay_ms = delay_ms_numer/delay_ms_denom;
            let dynamic_frame = DynamicImage::ImageRgba8(frame.into_buffer());
            // let resized_frame = dynamic_frame.resize(400, 400, FilterType::Lanczos3).to_rgba8();
            let resized_frame = dynamic_frame.thumbnail(400, 400).to_rgba8();
            encoder.add_frame(&resized_frame, current_timestamp_ms).ok()?;
            current_timestamp_ms += delay_ms as i32;
        }
        let webp_data = encoder.finalize(current_timestamp_ms).ok()?;
        return Some(webp_data.to_vec());
    } else {
        let img = image::load_from_memory(raw_bytes).ok()?;
        let img = img.thumbnail(400, 400);
        let mut webp_buffer = Vec::new();
        img.write_to(&mut Cursor::new(&mut webp_buffer), ImageFormat::WebP).ok()?;
        return Some(webp_buffer);
    }
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
pub async fn cleanup_log_directory<P: AsRef<Path>>(dir_path: P, state: &AppState) -> std::io::Result<()> {
    let dir = dir_path.as_ref();
    let index_path = dir.join("index.html");
    // No index.html = log wasn't uploaded in the end; just delete the entire dir
    if !index_path.exists() {
        fs::remove_dir_all(dir)?;
        return Ok(());
    }
    let html_content = fs::read_to_string(&index_path)?;
    let mut linked_files = HashSet::new();
    {
        let document = scraper::Html::parse_document(&html_content);
        // Look for tags that use 'src' attributes (img, video, audio, source)
        let src_selector = scraper::Selector::parse("[src]").unwrap();
        for element in document.select(&src_selector) {
            if let Some(src_val) = element.value().attr("src") {
                if let Some(filename) = Path::new(src_val).file_name() {
                    let filename = filename.to_string_lossy().into_owned();
                    linked_files.insert(filename);
                }
            }
        }
        let mut all_files = fs::read_dir(dir)?;
        while let Some(file) = all_files.next() {
            let file_path = file?.path();
            let file_name = file_path.file_name().unwrap().to_string_lossy().into_owned(); // it's safe to unwrap because we're already just reading the existing dir
            if file_name == "index.html" || file_name == "index.md" || get_extension(&file_name) == Some("tmp") { continue; }
            let file_name = askama::filters::urlencode(file_name).unwrap().to_string();
            if !linked_files.contains(&file_name) {
                fs::remove_file(file_path)?;
                linked_files.remove(&file_name);
            }
        }
    }
    for file in linked_files {
        if media_type(&file) == MediaType::Video {
            // put a job to compress
            println!("COMPRESS QUEUING {}", dir.join(&file).to_str().unwrap());
            let job = CompressVideoJob { path: dir.join(&file).to_string_lossy().into_owned(), created_on: SystemTime::now() };
            match state.tx.send(job).await {
                Ok(_) => {}
                Err(_) => { println!("Could not queue a video to compress: {}", file); }
            }
        }
    }
    Ok(())
}

pub async fn cleanup_all_log_directories(state: AppState) -> std::io::Result<()> {
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
                cleanup_log_directory(logdir.path(), &state).await?;
            }
        }
    }
    Ok(())
}

/* Compressing videos */

#[derive(Debug)]
pub struct CompressVideoJob {
    pub path: String,
    pub created_on: SystemTime,
}

pub async fn compress_video(CompressVideoJob{path, created_on}: CompressVideoJob) {
    let ext = match get_extension(&path) { Some(e) => e, None => { println!("COMPRESS FAIL: could not get extension of {}", path); return; }};
    let status = match ext {
        "mp4" => compress_mp4(&path).await,
        _ => Err(std::io::Error::other(format!("COMPRESS FAIL: not a recognized extension: {}", ext)))
    };
    match status {
        Ok(_) => {
            let tmp_file = format!("{}.tmp", path);
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if modified <= created_on {
                        let _ = fs::rename(tmp_file, &path);
                        println!("COMPRESS SUCCESS: {}", &path);
                        return;
                    }
                }
            }
            let _ = tokio::fs::remove_file(tmp_file).await;
            println!("COMPRESS WARN: compressed success, but original file was deleted or replaced: {}", path);
        }
        Err(e) => { println!("COMPRESS FAIL: {}", e); }
    }
}

/*
nice -n 19 ffmpeg -threads 1 -i btd0log1.mp4 -vf "scale='min(1920,iw)':-2" -c:v libx264 -preset veryfast -crf 24 -x264opts rc-lookahead=15 -c:a aac -b:a 128k -f mp4 btd0log1compressed.mp4
*/

async fn compress_mp4(input_path: &str) -> std::io::Result<()> {
    let status = Command::new("nice")
        .args([
            "-n", "19", 
            "ffmpeg", 
            "-threads", "1", 
            "-i", input_path,
            "-vf", "scale='min(1920,iw)':-2",
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-crf", "24",
            "-x264opts", "rc-lookahead=15",
            "-c:a", "aac",
            "-b:a", "128k",
            "-f", "mp4",
            "-y",
            format!("{}.tmp", input_path).as_str(),
        ])
        .stdout(std::process::Stdio::from(std::fs::OpenOptions::new().create(true).append(true).open("ffmpeg.log").unwrap()))
        .stderr(std::process::Stdio::from(std::fs::OpenOptions::new().create(true).append(true).open("ffmpeg.log").unwrap()))
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("FFmpeg execution failed"))
    }
}