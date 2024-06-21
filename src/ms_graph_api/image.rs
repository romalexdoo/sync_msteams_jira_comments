use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use regex::Regex;
use reqwest::header::HeaderMap;
use uuid::Uuid;

use crate::utils::get_reqwest_client;

#[derive(Debug)]
pub(crate) struct GraphApiImage {
    pub(crate) name: String,
    pub(crate) data: Vec<u8>,
    pub(crate) mime_str: String,
}

impl GraphApiImage {
    pub(crate) async fn get(access_token: &String, url: &String) -> Result<Self> {
        let client = get_reqwest_client()?;

        let response = client
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to send search issue request")?
            .error_for_status()
            .context("Search request bad status")?;

        let headers = response.headers().clone();
        
        let img = Self {
            name: format!("{}.{}", get_teams_attachment_id(url), get_image_extension(&headers)),
            data: response.bytes().await?.to_vec(),
            mime_str: headers.get("Content-Type").map_or(String::new(), |h| h.to_str().unwrap_or_default().to_string()),
        };

        Ok(img)
    }
}

fn get_teams_attachment_id(url: &String) -> String {
    extract_hosted_contents(url)
        .first()
        .map(|e| get_image_id(e).ok())
        .flatten()
        .unwrap_or(Uuid::new_v4().to_string())
}

fn extract_hosted_contents(text: &str) -> Vec<String> {
    // This pattern is designed to capture the "hostedContents" part of URLs
    // It assumes that the part of interest is right after "messages/" and continues until a double quote or space
    let re = Regex::new(r#"https://graph\.microsoft\.com/v1\.0/teams/[^\s\"]+/messages/[^\s\"]+/hostedContents/([^\s\|\]\\\"\/]+)"#).unwrap();

    re.captures_iter(text)
        .filter_map(|cap| {
            cap.get(1).map(|match_| match_.as_str().to_string())
        })
        .collect()
}

fn get_image_id(encoded: &String) -> Result<String> {
    let decoded = URL_SAFE.decode(encoded).context("Failed to decode string")?;

    let re = Regex::new(r"id=([^,]+)").unwrap();
    let id = re
        .captures(String::from_utf8(decoded).context("Failed to convert bytes to string")?.as_str())
        .and_then(|caps| 
            caps
                .get(1)
                .map(|id_match| 
                    id_match
                        .as_str()
                        .to_string()
                )
        )
        .ok_or(anyhow!("Failed to get ID from decoded string"))?;
    
    Ok(id)
}

fn get_image_extension(headers: &HeaderMap) -> String {
    let mut extension = "ext";
    
    if let Some(content_type) = headers.get("Content-Type") {
        if let Ok(content_type_str) = content_type.to_str() {
            extension = match content_type_str {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/gif" => "gif",
                "image/svg+xml" => "svg",
                _ => "ext",                
            }
        }
    }

    extension.to_string()
}
