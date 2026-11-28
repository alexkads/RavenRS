//! How the panel says statuses, durations and times, the same on every screen.

use ravenrs::prelude::{IconName, Tone};

/// A build's status, as people read it: `succeeded` is "Passou".
pub fn status_words(status: &str) -> &'static str {
    match status {
        "queued" => "Na fila",
        "running" => "Rodando",
        "succeeded" => "Passou",
        "failed" => "Falhou",
        "unstable" => "Instável",
        "cancelled" => "Cancelado",
        "timed_out" => "Passou do prazo",
        "interrupted" => "Interrompido",
        _ => "Sem build",
    }
}

/// The color of its own of a build whose agent went away (`interrupted`): neither a failure's nor
/// a cancellation's, as the coordination of the agents (Fase 3) asked.
pub const INTERRUPTED: ravenrs::prelude::Color = ravenrs::prelude::Color::rgb(0.62, 0.45, 0.92);

/// Whether a build with this status ended.
pub fn ended(status: &str) -> bool {
    !matches!(status, "queued" | "running" | "")
}

/// The tone of a build's badge.
pub fn status_tone(status: &str) -> Tone {
    match status {
        "running" => Tone::Accent,
        "succeeded" => Tone::Success,
        "failed" | "timed_out" => Tone::Danger,
        "queued" | "unstable" => Tone::Warning,
        _ => Tone::Neutral,
    }
}

/// The icon of a build's badge.
pub fn status_icon(status: &str) -> IconName {
    match status {
        "running" => IconName::LoaderCircle,
        "succeeded" => IconName::Check,
        "failed" | "timed_out" => IconName::X,
        "queued" => IconName::Clock,
        "unstable" => IconName::TriangleAlert,
        "interrupted" => IconName::Zap,
        _ => IconName::Minus,
    }
}

/// A stage's status (`done`, `failed`, `running`, `pending`), as people read it.
pub fn stage_words(status: &str) -> &'static str {
    match status {
        "done" => "passou",
        "failed" => "falhou",
        "running" => "rodando",
        "skipped" => "pulado",
        "cancelled" => "cancelado",
        _ => "esperando",
    }
}

/// A duration: "48 s", "3 min 40 s", "1 h 05 min".
pub fn duration(ms: i64) -> String {
    let seconds = (ms.max(0) + 500) / 1000;
    if seconds < 60 {
        return format!("{seconds} s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes} min {:02} s", seconds % 60);
    }
    format!("{} h {:02} min", minutes / 60, minutes % 60)
}

/// How long ago `then_ms` was at `now_ms`: "agora", "há 12 min", "há 3 h", "ontem", "há 4 dias".
pub fn ago(then_ms: i64, now_ms: i64) -> String {
    let seconds = (now_ms - then_ms).max(0) / 1000;
    match seconds {
        0..=59 => "agora".to_owned(),
        60..=3599 => format!("há {} min", seconds / 60),
        3600..=86_399 => format!("há {} h", seconds / 3600),
        86_400..=172_799 => "ontem".to_owned(),
        _ => format!("há {} dias", seconds / 86_400),
    }
}

/// Bytes as people read them: "38,4 MB".
pub fn bytes(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1.0 { format!("{mb:.1} MB").replace('.', ",") } else { format!("{} KB", bytes / 1024) }
}

/// The hour and minute of `ms`, on the clock of the server (UTC).
pub fn clock(ms: i64) -> String {
    let minutes = ms.div_euclid(60_000);
    format!("{:02}:{:02}", minutes.div_euclid(60).rem_euclid(24), minutes.rem_euclid(60))
}

/// A plugin's origin as the list says it.
pub fn origin_words(origin: &serde_json::Value) -> String {
    match origin {
        serde_json::Value::String(s) if s == "embedded" => "Embutido".to_owned(),
        serde_json::Value::Object(o) if o.contains_key("external") => "Externo".to_owned(),
        _ => "—".to_owned(),
    }
}

/// What a plugin's memory says: shared, stopped, its bytes or unknown.
pub fn memory_words(memory: &serde_json::Value) -> String {
    match memory["kind"].as_str() {
        Some("shared") => "no servidor".to_owned(),
        Some("not_running") => "0 · parado".to_owned(),
        Some("bytes") => memory["bytes"].as_u64().map_or_else(|| "—".to_owned(), bytes),
        _ => "—".to_owned(),
    }
}

/// A plugin's state: "ligado", "desligado", "recusado: …".
pub fn plugin_state(state: &serde_json::Value) -> (String, Tone) {
    match state["state"].as_str() {
        Some("on") => ("Ligado".to_owned(), Tone::Success),
        Some("off") => ("Desligado".to_owned(), Tone::Neutral),
        Some("refused") => ("Recusado".to_owned(), Tone::Danger),
        _ => ("—".to_owned(), Tone::Neutral),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_times_are_said_as_people_read_them() {
        assert_eq!(duration(48_000), "48 s");
        assert_eq!(duration(220_000), "3 min 40 s");
        assert_eq!(duration(3_900_000), "1 h 05 min");
        assert_eq!(ago(0, 30_000), "agora");
        assert_eq!(ago(0, 12 * 60_000), "há 12 min");
        assert_eq!(ago(0, 30 * 3_600_000), "ontem");
        assert_eq!(bytes(40_265_318), "38,4 MB");
        assert_eq!(clock(15 * 3_600_000 + 7 * 60_000), "15:07");
        assert_eq!(status_words("succeeded"), "Passou");
        assert_eq!(stage_words("pending"), "esperando");
        assert_eq!(status_words("interrupted"), "Interrompido");
        assert!(ended("interrupted") && !ended("running"));
    }
}
