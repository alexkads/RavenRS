//! Numbers written in digits reach the voice in words, and a recognizer hears them back.

use ravenrs_ai::{
    HEARING_RATE, ModelFolder, SPEECH_RATE, SpeechEngine, SpeechSettings, Transcriber, model_by_id,
    spoken_pt_br,
};

/// Lowercase words without accents or punctuation, to compare what was said with what was heard.
/// Both sides go through [`spoken_pt_br`], since a recognizer may write "R$ 12,50" for what it
/// heard as words.
fn words(text: &str) -> Vec<String> {
    let plain: String = spoken_pt_br(text)
        .to_lowercase()
        .chars()
        .map(|letter| match letter {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            letter if letter.is_alphanumeric() => letter,
            _ => ' ',
        })
        .collect();
    plain.split_whitespace().map(str::to_owned).collect()
}

/// #96: Kokoro says money, a date, a time and a percentage written in digits, and Whisper hears
/// the words. Needs `FF_AI_MODELS` with `kokoro-82m` and `whisper-base`; run it in release.
#[test]
#[ignore = "needs FF_AI_MODELS with kokoro-82m and whisper-base"]
fn whisper_hears_the_numbers_kokoro_says() {
    let root = std::env::var("FF_AI_MODELS").expect("FF_AI_MODELS");
    let folder = ModelFolder::new(root);
    let mut engine = SpeechEngine::load(&folder, &model_by_id("kokoro-82m").expect("known")).expect("Kokoro");
    let mut ears = Transcriber::load(&folder, &model_by_id("whisper-base").expect("known")).expect("Whisper");
    for text in [
        "A conta de luz de R$ 234,50 vence em 15/12.",
        "A reunião é às 14h30 e o desconto é de 15%.",
        "São 1.250 pessoas, três vírgula cinco por cento a mais.",
    ] {
        let mut sound = Vec::new();
        for piece in engine.pieces(text) {
            sound.extend(engine.speak(&piece, &SpeechSettings::default(), &mut |_| true).expect("sound"));
        }
        // Whisper hears at its own rate; the voice speaks at 24 kHz.
        let step = SPEECH_RATE as f32 / HEARING_RATE as f32;
        let heard_rate: Vec<f32> = (0..(sound.len() as f32 / step) as usize)
            .map(|i| {
                let at = i as f32 * step;
                let (a, f) = (at as usize, at.fract());
                sound[a] * (1.0 - f) + sound.get(a + 1).copied().unwrap_or(0.0) * f
            })
            .collect();
        let heard = ears.transcribe(&heard_rate, "pt").expect("heard").text;
        let (expected, got) = (words(text), words(&heard));
        let found = expected.iter().filter(|word| got.contains(word)).count();
        println!("{found}/{} | {text} | {heard}", expected.len());
        assert!(found * 10 >= expected.len() * 8, "said {text:?}, heard {heard:?}");
    }
}

#[test]
fn no_digit_reaches_the_voice() {
    use ravenrs_ai::spoken_pt_br as said;
    assert_eq!(said("R$ 2,50"), "dois reais e cinquenta centavos");
    assert!(!said("às 9h, 3,5%").chars().any(|letter| letter.is_ascii_digit()));
}
