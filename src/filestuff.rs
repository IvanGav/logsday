use std::io::Cursor;
use image::ImageFormat;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

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
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "avif" /* | "svg" */ => MediaType::Image,
        "mp4" | "mpeg" | "webm" | "ogv" | "mov" => MediaType::Video,
        "mp3" | "wav" | "oga" | "weba" => MediaType::Audio,
        _ => MediaType::Unsupported,
    }
}

/// Given a mime type, return its kind
/// [list](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/MIME_types/Common_types)
pub fn mime_media_type(mime_type: &str) -> MediaType {
    let mime_type = mime_type.split_once(';').unwrap_or((mime_type, "")).0; // get rid of parameters: `type/subtype;parameter=value`
    match mime_type.to_lowercase().as_str() {
        "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "image/avif" | "image/svg+xml" => MediaType::Image,
        "video/mp4" | "video/mpeg" | "video/webm" | "video/ogg" | "video/quicktime" => MediaType::Video,
        "audio/mpeg" | "audio/wav" | "audio/ogg" | "audio/webm" => MediaType::Audio,
        _ => MediaType::Unsupported,
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
    Ok(webp_buffer)
}

// disallow embedding html
pub fn render_markdown_to_html(markdown_input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_SUPERSCRIPT);
    options.insert(Options::ENABLE_SUBSCRIPT);

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