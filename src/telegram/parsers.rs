use crate::models::File;
use rust_tdlib::types::{FileType, FormattedText, MessageContent, TextEntity, TextEntityType, File as TgFile};

pub fn parse_message_content(message: &MessageContent) -> Option<(Option<String>, Option<File>)> {
    match message {
        MessageContent::MessageText(text) => Some((Some(parse_formatted_text(text.text())), None)),
        MessageContent::MessageAnimation(message_animation) => {
            Some((
                Some(parse_formatted_text(message_animation.caption())),
                Some(message_animation.animation().into()),
            ))
        }
        MessageContent::MessageAudio(message_audio) => None,
        MessageContent::MessageDocument(message_document) => None,
        MessageContent::MessagePhoto(photo) => {
            let file = match photo.photo().sizes().first() {
                None => None,
                Some(photo) => Some(photo.photo().into()),
            };
            Some((Some(parse_formatted_text(photo.caption())), file))
        }
        MessageContent::MessageVideo(message_video) => None,


        MessageContent::MessageChatChangePhoto(_) => None,

        MessageContent::MessagePoll(_) => None,
        MessageContent::MessageChatChangeTitle(_) => None,
        MessageContent::MessageChatDeletePhoto(_) => None,
        MessageContent::MessageChatJoinByLink(_) => None,
        MessageContent::MessageChatUpgradeFrom(_) => None,
        MessageContent::MessageChatUpgradeTo(_) => None,
        MessageContent::MessageContact(_) => None,
        MessageContent::MessageContactRegistered(_) => None,
        MessageContent::MessageCustomServiceAction(_) => None,
        MessageContent::MessageExpiredPhoto(_) => None,
        MessageContent::MessageExpiredVideo(_) => None,
        MessageContent::MessageInvoice(_) => None,
        MessageContent::MessageLocation(_) => None,
        MessageContent::MessagePassportDataReceived(_) => None,
        MessageContent::MessageScreenshotTaken(_) => None,
        MessageContent::MessageSticker(message_sticker) => {
            let file = TelegramFileWithMeta {
                path: message_sticker.sticker().sticker().into(),
                file_type: FileType::Image(message_sticker.sticker().into()),
                file_name: None,
            };
            Some((None, Some(file)))
        }
        MessageContent::MessageSupergroupChatCreate(_) => None,

        MessageContent::MessageVenue(_) => None,

        MessageContent::MessageVideoNote(message_video_note) => {
            let file = TelegramFileWithMeta {
                path: message_video_note.video_note().video().into(),
                file_type: FileType::Video(message_video_note.video_note().into()),
                file_name: None,
            };
            Some((None, Some(file)))
        }
        MessageContent::MessageVoiceNote(_) => None,
        MessageContent::MessageWebsiteConnected(_) => None,

        MessageContent::_Default => None,
        MessageContent::MessageBasicGroupChatCreate(_) => None,
        MessageContent::MessageCall(_) => None,
        MessageContent::MessageChatAddMembers(_) => None,
        MessageContent::MessageChatDeleteMember(_) => None,
        MessageContent::MessageChatSetTtl(_) => None,
        MessageContent::MessageGame(_) => None,
        MessageContent::MessageGameScore(_) => None,
        MessageContent::MessagePassportDataSent(_) => None,
        MessageContent::MessagePaymentSuccessful(_) => None,
        MessageContent::MessagePaymentSuccessfulBot(_) => None,
        MessageContent::MessagePinMessage(_) => None,
        MessageContent::MessageUnsupported(_) => None,
        MessageContent::MessageDice(_) => None,
        MessageContent::MessageProximityAlertTriggered(_) => None,
    }
}

pub fn parse_formatted_text(formatted_text: &FormattedText) -> String {
    let mut entities_by_index = make_entities_stack(formatted_text.entities());
    let mut result_text = String::new();
    let mut current_entity = match entities_by_index.pop() {
        None => return formatted_text.text().clone(),
        Some(entity) => entity,
    };
    for (i, ch) in formatted_text.text().chars().enumerate() {
        if i == current_entity.0 {
            result_text = format!("{}{}{}", result_text, current_entity.1, ch);
            current_entity = match entities_by_index.pop() {
                None => {
                    result_text = format!(
                        "{}{}",
                        result_text,
                        &formatted_text
                            .text()
                            .chars()
                            .skip(i + 1)
                            .take(formatted_text.text().len() - i)
                            .collect::<String>()
                    );
                    return result_text;
                }
                Some(entity) => entity,
            };
        } else {
            result_text.push(ch)
        }
    }
    result_text
}

fn make_entities_stack(entities: &[TextEntity]) -> Vec<(usize, String)> {
    let mut stack = Vec::new();
    for entity in entities {
        let formatting = match entity.type_() {
            TextEntityType::Bold(_) => Some(("<b>".to_string(), "</b>".to_string())),
            TextEntityType::Code(_) => Some(("<code>".to_string(), "</code>".to_string())),
            TextEntityType::Hashtag(_) => Some(("#".to_string(), "".to_string())),
            TextEntityType::Italic(_) => Some(("<i>".to_string(), "</i>".to_string())),
            TextEntityType::PhoneNumber(_) => Some(("<phone>".to_string(), "</phone>".to_string())),
            TextEntityType::Pre(_) => Some(("<pre>".to_string(), "</pre>".to_string())),
            TextEntityType::PreCode(_) => {
                Some(("<pre><code>".to_string(), "</code></pre>".to_string()))
            }
            TextEntityType::Strikethrough(_) => {
                Some(("<strike>".to_string(), "</strike>".to_string()))
            }
            TextEntityType::TextUrl(u) => {
                let tag = format!(r#"<a href="{}">"#, u.url());
                Some((tag, "</a>".to_string()))
            }
            TextEntityType::Underline(_) => Some(("<u>".to_string(), "</u>".to_string())),
            TextEntityType::Url(_) => Some(("<a>".to_string(), "</a>".to_string())),
            TextEntityType::_Default => None,
            TextEntityType::BotCommand(_) => None,
            TextEntityType::Cashtag(_) => None,
            TextEntityType::EmailAddress(_) => None,
            TextEntityType::Mention(_) => None,
            TextEntityType::MentionName(_) => None,
            TextEntityType::BankCardNumber(_) => None,
        };
        if let Some((start_tag, end_tag)) = formatting {
            stack.push((entity.offset() as usize, start_tag));
            stack.push(((entity.offset() + entity.length()) as usize, end_tag));
        }
    }
    stack.sort_by_key(|(i, _)| *i);
    stack.reverse();
    stack
}


impl From<&TgFile> for File {
    fn from(file: &TgFile) -> Self {
        Self {
            local_path: Some(file.local().path().as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            remote_file: file.id().to_string(),
            remote_id: file.remote().unique_id().clone(),
        }
    }
}