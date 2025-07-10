use anyhow::{Context, Result};
use regex::Regex;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use crate::ms_graph_api::{
    image::GraphApiImage, message::TeamsAttachment
};

use super::{issue::Issue, model::JiraAPIShared};


#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JiraAttachment {
    pub(crate) id: String,
    pub(crate) filename: String,
}

pub(crate) fn add_attachments_urls_to_description(description: &mut String, attachments: &Vec<TeamsAttachment>) -> bool {
    let mut updated = false;

    for attachment in attachments {
        if let (Some(content_url), Some(name)) = (&attachment.content_url, &attachment.name) {
            if !description.contains(content_url) {
                description.push_str(format!("\n\n[{}|{}]", name, content_url).as_str());
                updated = true;
            }
        }
    }

    updated
}

pub(crate) async fn replace_images_in_description(
    description: &mut String, 
    graph_api_token: &String, 
) -> Result<Vec<GraphApiImage>> {
    let url_regex = Regex::new(r#"https://graph\.microsoft\.com/v1\.0/[^\s\|\]\\\"]*"#).unwrap();
    let mut urls: Vec<_> = url_regex.find_iter(description)
        .map(|mat| mat.as_str().to_string())
        .collect();

    let mut result: Vec<GraphApiImage> = Vec::new();

    if urls.len() > 0 {
        urls.sort();
        urls.dedup();
        
        for url in urls {
            if let Ok(img) = GraphApiImage::get(graph_api_token, &url).await {
                *description = replace_img_tag_for_jira(description, &url, &img.name);
                result.push(img);
            }
        }
    }

    Ok(result)
}

pub(crate) async fn replace_attachments(
    jira_api: &JiraAPIShared, 
    issue: &Issue, 
    old_image_names: &Vec<String>, 
    new_images: &Vec<GraphApiImage>
) -> Result<()> {
    let old_attachments = issue.get_attachments();

    for old_image_name in old_image_names {
        if !new_images.iter().any(|i| i.name == *old_image_name) {
            if let Some(attachments) = old_attachments.as_ref() {
                if let Some(attachment) = attachments.iter().find(|a| a.filename == *old_image_name) {
                    let _ = delete_attachment(jira_api, &attachment.id).await;
                }
            }
        }
    }

    for image in new_images {
        if old_attachments.as_ref().map_or(true, |v| !v.iter().any(|a| a.filename == image.name)) {
            let _ = upload_image(jira_api, issue, image).await;
        }
    }

    Ok(())
}

async fn upload_image(jira_api: &JiraAPIShared, issue: &Issue, image: &GraphApiImage) -> Result<()> {
    let img_data = Part::bytes(image.data.clone())
        .file_name(image.name.clone())
        .mime_str(&image.mime_str)?;

    let form = Form::new().part("file", img_data);

    jira_api.client
        .post(format!("{}/rest/api/2/issue/{}/attachments", jira_api.config.base_url, issue.get_id()))
        .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
        .header("X-Atlassian-Token", "no-check") // Add the X-Atlassian-Token header
        .multipart(form)
        .send()
        .await
        .context("Failed to send upload image request")?
        .error_for_status()
        .context("Upload image request bad status")?;

    Ok(())
}


fn replace_img_tag_for_jira(text: &String, search_url: &String, replace_with: &String) -> String {
    // Escaping the target URL to safely insert it into the regex pattern
    let escaped_url = regex::escape(search_url);

    // Crafting a regex pattern that matches an <img> tag containing the specific URL
    // The pattern loosely captures an <img> tag to ensure flexibility in attribute ordering and spacing
    let pattern = format!(r#"<img[^>]*?src\s*=\s*['"]?{}['"]?[^>]*?/?>"#, escaped_url);

    // Compiling the regex pattern
    let re = Regex::new(&pattern).expect("Invalid regex pattern");

    // Replacing the matched <img> tag(s) with an empty string
    // You can replace "" with any other placeholder text if needed
    re.replace_all(text, format!("\n\n!{}!\n\n", replace_with)).into_owned()
}

async fn delete_attachment(jira_api: &JiraAPIShared, attachment_id: &String) -> Result<()> {
    jira_api.client
        .delete(format!("{}/rest/api/2/attachment/{}", jira_api.config.base_url, attachment_id))
        .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
        .send()
        .await
        .context("Failed to send delete attachment request")?
        .error_for_status()
        .context("Delete attachment request bad status")?;

    Ok(())
}

pub(crate) fn find_old_attached_images(description: &String) -> Vec<String> {
    let pattern_str = format!("\n\n!+([^!]+)!\n\n");
    let pattern = Regex::new(&pattern_str).unwrap();
    
    pattern
        .find_iter(description)
        .map(|mat| {
                let chars = mat.as_str().chars().collect::<Vec<_>>();
                if chars.len() > 6 {
                    let trimmed_range = chars.iter().skip(3).take(chars.len() - 6);
                    trimmed_range.collect()
                } else {
                    String::new()
                }
            }
        )
        .filter(|m| !m.is_empty())
        .collect()
}