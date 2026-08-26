use std::process::Stdio;

use tokio::{io::AsyncWriteExt, process::Command};

use crate::{AppState, db};

pub const GMAIL_ADDRESS: &str = "logsday.mail@gmail.com";

// panic with an error message if the right env variables have not been set
pub fn ensure_env_variables() {
    if !cfg!(debug_assertions) {
        let _ = get_app_password();
    }
}

pub async fn send_emails_to_users(state: &AppState) -> usize {
    let emails = db::get_all_user_emails_whose_logsday_is_today(state).await;
    if cfg!(debug_assertions) {
        println!("Run in release mode to actually send emails, or change an assert in `email.rs`");
        for email in &emails {
            println!("Would send email to {}", &email);
        }
        return emails.len();
    } else {
        let mut sent = 0;
        for email in &emails {
            if let Err(e) = send_gmail(email, "Logsday", "Hey Logger. Logsday.").await {
                println!("Error sending email to {} - {}", email, e);
            } else {
                sent += 1;
            }
        }
        return sent;
    }
}

fn get_app_password() -> String {
    return std::env::var("GMAIL_APP_PASSWORD").expect("Please set GMAIL_APP_PASSWORD to your gmail app password (need to have 2FA set up for the account)");
}

pub async fn send_gmail(to_email: &str, subject: &str, body_text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let email_raw = format!(
        "From: Logsday <{GMAIL_ADDRESS}>\r\n\
        To: {}\r\n\
        Subject: {}\r\n\r\n\
        {}",
        to_email, subject, body_text
    );

    let mut child = Command::new("curl")
        .args([
            "--url", "smtps://smtp.gmail.com:465",
            "--ssl-reqd",
            "--mail-from", GMAIL_ADDRESS,
            "--mail-rcpt", to_email,
            "--user", &format!("{}:{}", GMAIL_ADDRESS, get_app_password()),
            "-T", "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(email_raw.as_bytes()).await?;
    }

    let output = child.wait_with_output().await?;
    if !output.status.success() {
        println!("curl error: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}