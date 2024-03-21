use regex::Regex;
use serde::Deserialize;
use std::{fs::OpenOptions, io::Write};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationTokenQuery {
    pub validation_token: String,
}

pub fn log_to_file(handler: &str, payload: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("test.txt") {
            let data = format!("{}\t{}\t{}\n", chrono::Local::now().format("%d.%m.%Y %H:%M:%S").to_string(), handler, payload);
            let _ = file.write_all(data.as_bytes());
    };
}

pub fn get_message_id_and_reply_id(resource: &String) -> (Option<String>, Option<String>) {
    let mut message_id = None;
    let mut reply_id = None;
    for part in resource.rsplit("/") {
        if part.starts_with("replies") {
            reply_id = get_id(part);
        } else if part.starts_with("messages") {
            message_id = get_id(part);
            break;
        };
    }
    
    (message_id, reply_id)
}

fn get_id(text: &str) -> Option<String> {
    let re = Regex::new(r"\w+\('([^']*)'\)").unwrap();

    re.captures(text)
        .and_then(|cap| cap.get(1).map(|match_| match_.as_str().to_string()))
}