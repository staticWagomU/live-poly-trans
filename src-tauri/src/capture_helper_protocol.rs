use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CaptureHelperMessage {
    Start {
        stream: String,
    },
    #[serde(rename_all = "camelCase")]
    AudioFrame {
        stream: String,
        pcm16_base64: String,
        sample_rate: u32,
        timestamp_ms: i64,
    },
    Stop {
        stream: String,
    },
}

impl CaptureHelperMessage {
    pub fn parse_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }

    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_start_message() {
        assert_eq!(
            CaptureHelperMessage::parse_line(r#"{"type":"start","stream":"mic"}"#).unwrap(),
            CaptureHelperMessage::Start {
                stream: "mic".to_string()
            }
        );
    }

    #[test]
    fn parses_audio_frame_message_with_windows_ready_fields() {
        assert_eq!(
            CaptureHelperMessage::parse_line(
                r#"{"type":"audioFrame","stream":"speaker","pcm16Base64":"AAE=","sampleRate":16000,"timestampMs":250}"#
            )
            .unwrap(),
            CaptureHelperMessage::AudioFrame {
                stream: "speaker".to_string(),
                pcm16_base64: "AAE=".to_string(),
                sample_rate: 16_000,
                timestamp_ms: 250
            }
        );
    }

    #[test]
    fn serializes_stop_message() {
        let line = CaptureHelperMessage::Stop {
            stream: "mic".to_string(),
        }
        .to_json_line()
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(value, json!({"type": "stop", "stream": "mic"}));
    }
}
