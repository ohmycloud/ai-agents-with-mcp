//! Convert MCP tool content blocks to the text representation used in the
//! original course's Anthropic tool loop.

use rmcp::model::{ContentBlock, ResourceContents};

/// Extracts text and base64 payload strings from MCP result content.
///
/// The source Python client exposes text, image/audio data, and embedded
/// resource text/blob as strings before sending them back to the model.
pub fn content_as_strings(content: &[ContentBlock]) -> Vec<String> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.text.clone()),
            ContentBlock::Image(image) => Some(image.data.clone()),
            ContentBlock::Audio(audio) => Some(audio.data.clone()),
            ContentBlock::Resource(resource) => match &resource.resource {
                ResourceContents::TextResourceContents { text, .. } => Some(text.clone()),
                ResourceContents::BlobResourceContents { blob, .. } => Some(blob.clone()),
                _ => None,
            },
            ContentBlock::ResourceLink(_) => None,
            _ => None,
        })
        .collect()
}
