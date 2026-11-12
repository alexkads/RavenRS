//! Brazilian Portuguese written as the phonemes Kokoro reads: where the stress falls,
//! which vowels open, close or nasalize, and how a word changes before the next.
//!
//! The rules follow the spelling: accents mark the stress, the ending decides it when
//! there is none, and a short list keeps the open vowels that spelling does not show.

/// Words said apart from the rules, as they sound in a sentence.
const SPOKEN: &[(&str, &str)] = &[
    ("a", "a"),
    ("à", "ˌaː"),
    ("às", "aːs"),
    ("ao", "W"),
    ("aos", "Ws"),
    ("as", "as"),
    ("até", "atˌɛ"),
    ("com", "koŋ"),
    ("como", "kˌomʊ"),
    ("da", "da"),
    ("das", "das"),
    ("de", "ʤy"),
    ("dela", "dˌɛlæ"),
    ("do", "dʊ"),
    ("dos", "dʊs"),
    ("e", "i"),
    ("é", "ɛ"),
    ("ela", "ˌɛlæ"),
    ("elas", "ˌɛlæs"),
    ("ele", "ˌely"),
    ("eles", "ˌelys"),
    ("em", "ˈAŋ"),
    ("essa", "ˈɛsæ"),
    ("esse", "ˌesi"),
    ("esta", "ˌɛstæ"),
    ("está", "estˌa"),
    ("estão", "estˌɐ̃ʊ̃"),
    ("este", "ˌesʧy"),
    ("eu", "ˈeʊ"),
    ("lhe", "ʎy"),
    ("mas", "mas"),
    ("me", "my"),
    ("meu", "meʊ"),
    ("minha", "mˌiɲæ"),
    ("minhas", "mˌiɲæs"),
    ("muito", "mwˈiŋtʊ"),
    ("muitos", "mwˈiŋtʊs"),
    ("muita", "mwˈiŋtæ"),
    ("muitas", "mwˈiŋtæs"),
    ("na", "na"),
    ("nas", "nas"),
    ("não", "nˌɐ̃ʊ̃"),
    ("no", "nʊ"),
    ("nos", "nʊs"),
    ("o", "ʊ"),
    ("onde", "ˌoŋʤy"),
    ("os", "ʊs"),
    ("ou", "ow"),
    ("para", "pˌaɾæ"),
    ("pela", "pˈelæ"),
    ("pelas", "pˈelæs"),
    ("pelo", "pˈelʊ"),
    ("pelos", "pˈelʊs"),
    ("pode", "pˌɔʤy"),
    ("por", "por"),
    ("pra", "prˌa"),
    ("que", "ky"),
    ("se", "sy"),
    ("ser", "ser"),
    ("seu", "seʊ"),
    ("seus", "seʊs"),
    ("sua", "sˌuæ"),
    ("suas", "sˌuæs"),
    ("são", "sɐ̃ʊ̃"),
    ("te", "ʧy"),
    ("tem", "tAŋ"),
    ("ter", "ter"),
    ("um", "ũŋ"),
    ("uma", "ˌumæ"),
    ("umas", "ˌumæs"),
    ("uns", "ũŋs"),
    ("vai", "vI"),
    ("faz", "fas"),
    ("foi", "foɪ"),
    ("neste", "nˌesʧi"),
    ("nesta", "nˌɛstæ"),
    ("qualquer", "kwWkˈɛr"),
    // Words of other languages, as they are said here.
    ("backup", "bɛkˈæp"),
    ("command", "kommˈɐ̃ŋd"),
    ("control", "kˈoŋtrow"),
    ("drive", "drˈIv"),
    ("email", "imˈeɪʊ"),
    ("excel", "esˈɛʊ"),
    ("internet", "ˌiŋteɾənˈɛʧ"),
    ("k", "kˈa"),
    ("mac", "mˈɛk"),
    ("pen", "pˈAŋ"),
    ("windows", "wˈiŋdows"),
    ("você", "vosˌe"),
    ("vocês", "vosˌes"),
];

/// Words whose stressed `e` or `o` is open though the spelling does not mark it.
const OPEN: &[&str] = &[
    "dez",
    "loja",
    "lojas",
    "peça",
    "peças",
    "peço",
    "pegue",
    "pega",
    "pegam",
    "querem",
    "cobre",
    "cobrem",
    "caderno",
    "cadernos",
    "melhor",
    "melhores",
    "pior",
    "piores",
    "maior",
    "menor",
    "menores",
    "serve",
    "servem",
    "jogue",
    "joga",
    "jogam",
    "prefere",
    "preferem",
    "esquece",
    "esquecem",
    "sete",
    "pedra",
    "festa",
    "festas",
    "febre",
    "sério",
    "perna",
    "pernas",
    "velho",
    "velha",
    "velhas",
    "velhos",
    "nossa",
    "nosso",
    "nossos",
    "nossas",
    "vossa",
    "costa",
    "costas",
    "cola",
    "bola",
    "bolas",
    "roda",
    "rodas",
    "sobra",
    "sobram",
    "copo",
    "copos",
    "olha",
    "olham",
    "ovos",
    "porcos",
    "povos",
    "corpos",
    "moça",
    "moças",
    "roça",
    "forma",
    "formas",
    "norte",
    "forte",
    "sorte",
    "morte",
    "porte",
    "porta",
    "portas",
    "colega",
    "colegas",
    "neto",
    "neta",
    "netos",
    "netas",
    "terça",
    "época",
    "épocas",
    "ela",
    "elas",
    "fecham",
    "aberta",
    "abertas",
    "aberto",
    "abertos",
    "agora",
    "aluguel",
    "amarela",
    "amarelas",
    "aparece",
    "aparecem",
    "aperte",
    "bela",
    "belo",
    "certa",
    "certas",
    "certo",
    "certos",
    "coisa",
    "comece",
    "começa",
    "começam",
    "correm",
    "corre",
    "deve",
    "devem",
    "devolve",
    "devolvem",
    "escreve",
    "escrevem",
    "fecha",
    "fora",
    "hora",
    "horas",
    "interessa",
    "janela",
    "janelas",
    "leva",
    "levam",
    "logo",
    "mexe",
    "mexem",
    "modo",
    "moda",
    "mostra",
    "mostram",
    "nota",
    "notas",
    "nova",
    "novas",
    "maiores",
    "painel",
    "parcela",
    "parcelas",
    "pede",
    "pedem",
    "perto",
    "porta",
    "portas",
    "pode",
    "podem",
    "quer",
    "recebe",
    "recebem",
    "regra",
    "regras",
    "repete",
    "repetem",
    "reserva",
    "reservas",
    "seta",
    "setas",
    "sete",
    "tela",
    "telas",
    "terra",
    "troca",
    "trocam",
    "volta",
    "voltam",
    "volte",
    "zero",
    "zeros",
    "dela",
    "delas",
    "nela",
    "nelas",
    "papel",
    "hotel",
    "nível",
    "célula",
    "obra",
    "obras",
    "ótimo",
    "mesa",
    "mesas",
    "pé",
];

/// Endings whose stressed `e` or `o` is open in most words that have them.
fn opens_by_ending(word: &str) -> bool {
    const ENDINGS: &[&str] = &[
        "ola", "olas", "oja", "ojas", "orte", "ortes", "orta", "ortas", "ece", "ecem", "ere", "erem", "erve",
        "ervem", "ega", "egam", "egue", "eguem", "ol", "el", "éis", "óis", "ova", "ovas", "oca", "ocas",
        "eja",
    ];
    // Those that stay closed though they end the same.
    const CLOSED: &[&str] = &[
        "pela", "pelas", "aquela", "aquelas", "esta", "veja", "deseja", "seja", "sejam", "deseje", "planeja",
    ];
    !CLOSED.contains(&word)
        && ENDINGS.iter().any(|ending| word.ends_with(ending) && word.len() > ending.len())
}

/// Words whose `x` sounds as `ks`, or as `s`.
const X_AS_KS: &[&str] =
    &["fixa", "fixas", "fixo", "fixos", "táxi", "tóxico", "sexo", "anexo", "anexos", "anexar", "fixar"];
const X_AS_S: &[&str] =
    &["próximo", "próximos", "próxima", "próximas", "máximo", "máxima", "auxílio", "trouxe"];

fn is_vowel(letter: char) -> bool {
    matches!(base(letter), 'a' | 'e' | 'i' | 'o' | 'u')
}

/// The letter without its accent.
fn base(letter: char) -> char {
    match letter {
        'á' | 'à' | 'â' | 'ã' => 'a',
        'é' | 'ê' => 'e',
        'í' => 'i',
        'ó' | 'ô' | 'õ' => 'o',
        'ú' | 'ü' => 'u',
        'y' => 'i',
        other => other,
    }
}

fn marks_stress(letter: char) -> bool {
    matches!(letter, 'á' | 'à' | 'â' | 'é' | 'ê' | 'í' | 'ó' | 'ô' | 'ú')
}

/// Where each syllable's vowels are in the word: start and end, the end not included.
fn nuclei(letters: &[char]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut place = 0;
    while place < letters.len() {
        let letter = letters[place];
        // After q and g the u only says w, or nothing.
        let glide_u = base(letter) == 'u'
            && place > 0
            && matches!(letters[place - 1], 'q' | 'g')
            && place + 1 < letters.len()
            && is_vowel(letters[place + 1]);
        if !is_vowel(letter) || glide_u {
            place += 1;
            continue;
        }
        let start = place;
        let mut end = place + 1;
        // A falling diphthong: a vowel, then an unstressed i or u in the same syllable.
        if end < letters.len() {
            let next = letters[end];
            let first = base(letter);
            let pairs = matches!(
                (first, next),
                ('a', 'i' | 'u')
                    | ('e', 'i' | 'u')
                    | ('o', 'i' | 'u')
                    | ('u', 'i')
                    | ('i', 'u')
                    | ('ã', 'o' | 'e')
            ) || matches!((letter, next), ('ã', 'o' | 'e') | ('õ', 'e') | ('é' | 'ó', 'i' | 'u'));
            let closes = |at: usize| {
                // "sair", "ruim", "juiz": a consonant that ends the syllable makes a hiatus.
                at + 1 < letters.len()
                    && matches!(letters[at + 1], 'r' | 'l' | 'z' | 'm' | 'n')
                    && (at + 2 == letters.len() || !is_vowel(letters[at + 2]))
                    && !(letters[at + 1] == 'n' && letters.get(at + 2) == Some(&'h'))
            };
            let nh = end + 2 < letters.len() + 1
                && letters.get(end + 1) == Some(&'n')
                && letters.get(end + 2) == Some(&'h');
            let before_glide = next == 'i' && letters.get(end + 1) == Some(&'u');
            if pairs && !marks_stress(next) && !closes(end) && !nh && !before_glide {
                end += 1;
            }
        }
        found.push((start, end));
        place = end;
    }
    found
}

/// Which syllable takes the stress, by the accent or else by the ending.
fn stressed(letters: &[char], nuclei: &[(usize, usize)]) -> usize {
    if nuclei.is_empty() {
        return 0;
    }
    if let Some(index) =
        nuclei.iter().position(|(start, end)| letters[*start..*end].iter().any(|l| marks_stress(*l)))
    {
        return index;
    }
    if let Some(index) =
        nuclei.iter().position(|(start, end)| letters[*start..*end].iter().any(|l| matches!(l, 'ã' | 'õ')))
    {
        return index;
    }
    let last = nuclei.len() - 1;
    let word: String = letters.iter().collect();
    let mut stem = word.as_str();
    if let Some(cut) = stem.strip_suffix('s') {
        stem = cut;
    }
    let penultimate = stem.ends_with('a')
        || stem.ends_with('e')
        || stem.ends_with('o')
        || stem.ends_with("am")
        || stem.ends_with("em")
        || stem.ends_with("en");
    // An ending diphthong with i or u keeps the stress: "vai", "chapéu", "aconteceu".
    let (start, end) = nuclei[last];
    let ends_in_glide = end - start == 2 && end == letters.len() - usize::from(word.ends_with('s'));
    if penultimate && last > 0 && !ends_in_glide { last - 1 } else { last }
}

/// What follows a vowel, for its nasal sound.
fn nasal_coda(letters: &[char], after: usize) -> Option<bool> {
    let letter = *letters.get(after)?;
    if !matches!(letter, 'm' | 'n') {
        return None;
    }
    let next = letters.get(after + 1).copied();
    match next {
        None => Some(false),
        Some('h') if letter == 'n' => None,
        Some(next) if is_vowel(next) => None,
        Some(next) => Some(matches!(next, 'p' | 'b')),
    }
}

/// A word by the rules.
fn word_phonemes(word: &str) -> String {
    let letters: Vec<char> = word.chars().collect();
    let nuclei = nuclei(&letters);
    let stress = stressed(&letters, &nuclei);
    let open = OPEN.contains(&word) || opens_by_ending(word);
    let count = nuclei.len();
    let mut syllable_of = vec![usize::MAX; letters.len()];
    for (index, (start, end)) in nuclei.iter().enumerate() {
        for slot in &mut syllable_of[*start..*end] {
            *slot = index;
        }
    }
    // Secondary stress on the first syllable, and every other one, of a long word.
    let secondary = |index: usize| {
        count >= 3 && index.is_multiple_of(2) && index < stress && (index == 0 || index + 2 <= stress)
    };
    let mut out = String::new();
    let mut place = 0;
    let at = |place: usize| letters.get(place).copied();
    while place < letters.len() {
        let letter = letters[place];
        if let Some(index) = syllable_of.get(place).copied().filter(|index| *index != usize::MAX) {
            let (start, end) = nuclei[index];
            if place != start {
                place += 1;
                continue;
            }
            if index == stress {
                out.push('ˈ');
            } else if secondary(index) {
                out.push('ˌ');
            }
            let vowels: Vec<char> = letters[start..end].to_vec();
            // After the stress, an i before a vowel glides into it: "cópia", "relatório".
            // So does an e: "vídeo", "área".
            if index > stress
                && (vowels == ['i'] || vowels == ['e'])
                && letters.get(end).is_some_and(|n| is_vowel(*n))
            {
                out.push('j');
                place = end;
                continue;
            }
            let coda = nasal_coda(&letters, end);
            let is_final = index == count - 1;
            let final_of_word = end == letters.len() || (end + 1 == letters.len() && letters[end] == 's');
            out.push_str(&vowel(
                &vowels,
                index == stress,
                index > stress,
                is_final && final_of_word,
                coda,
                open,
                &letters,
                end,
            ));
            if coda.is_some() {
                place = end + 1;
                // "-am" at the end of a verb is a nasal diphthong.
                continue;
            }
            place = end;
            continue;
        }
        let next = at(place + 1);
        let next_is_front = next.is_some_and(|n| matches!(base(n), 'e' | 'i'));
        let previous_vowel = place > 0 && is_vowel(letters[place - 1]);
        let next_vowel = next.is_some_and(is_vowel);
        match letter {
            'b' | 'f' | 'k' | 'p' | 'v' | 'w' => out.push(letter),
            'c' if next == Some('h') => {
                out.push('ʃ');
                place += 1;
            }
            'c' | 'ç' if letter == 'ç' || next_is_front => out.push('s'),
            'c' => out.push('k'),
            'd' => out.push('d'),
            't' => out.push('t'),
            'g' if next == Some('u') && at(place + 2).is_some_and(|n| matches!(base(n), 'e' | 'i')) => {
                out.push('ɡ');
                place += 1;
            }
            'g' if next == Some('u') && at(place + 2).is_some_and(is_vowel) => {
                out.push_str("ɡw");
                place += 1;
            }
            'g' if next_is_front => out.push('ʒ'),
            'g' => out.push('ɡ'),
            'q' if next == Some('u') && at(place + 2).is_some_and(|n| matches!(base(n), 'e' | 'i')) => {
                out.push('k');
                place += 1;
            }
            'q' if next == Some('u') => {
                out.push_str("kw");
                place += 1;
            }
            'q' => out.push('k'),
            'h' => {}
            'j' => out.push('ʒ'),
            'l' if next == Some('h') => {
                out.push_str("lj");
                place += 1;
            }
            'l' if next_vowel => out.push('l'),
            'l' => {
                // At the end of a syllable it becomes a vowel.
                match out.chars().last() {
                    Some('ɔ') => out.push('l'),
                    Some('u') if letters[place - 1] == 'ú' => out.push('l'),
                    Some('a') => {
                        out.pop();
                        out.push('W');
                    }
                    Some('o') => out.push('w'),
                    Some('u') => out.push('w'),
                    _ => out.push('ʊ'),
                }
            }
            'm' | 'n' if next == Some('h') && letter == 'n' => {
                out.push('ɲ');
                place += 1;
            }
            'm' | 'n' => out.push(letter),
            'r' => {
                let doubled = next == Some('r');
                let initial = place == 0;
                let after_coda = place > 0 && matches!(letters[place - 1], 'n' | 'l' | 's');
                let in_cluster = place > 0 && !is_vowel(letters[place - 1]) && !after_coda;
                if doubled || initial || after_coda {
                    out.push('x');
                    if doubled {
                        place += 1;
                    }
                } else if in_cluster {
                    out.push('r');
                } else if next_vowel {
                    out.push('ɾ');
                } else if next.is_none() {
                    out.push('r');
                } else {
                    out.push_str("ɾə");
                }
            }
            's' if next == Some('s') => {
                out.push('s');
                place += 1;
            }
            's' if next == Some('c') && at(place + 2).is_some_and(|n| matches!(base(n), 'e' | 'i')) => {
                out.push('s');
                place += 1;
            }
            's' if previous_vowel && next_vowel => out.push('z'),
            's' if next
                .is_some_and(|n| matches!(n, 'b' | 'd' | 'g' | 'l' | 'm' | 'n' | 'r' | 'v' | 'z' | 'j')) =>
            {
                out.push('z');
            }
            's' => out.push('s'),
            'x' => {
                if X_AS_KS.contains(&word) {
                    out.push_str("ks");
                } else if X_AS_S.contains(&word) {
                    out.push('s');
                } else if place == 1 && letters[0] == 'e' {
                    if next_vowel {
                        out.push('z');
                    } else if next == Some('c') && at(place + 2).is_some_and(|n| matches!(base(n), 'e' | 'i'))
                    {
                        out.push('s');
                        place += 1;
                    } else {
                        out.push('s');
                    }
                } else if next.is_none() {
                    out.push_str("ks");
                } else {
                    out.push('ʃ');
                }
            }
            'z' if next.is_none() => out.push('s'),
            'z' => out.push('z'),
            other if other.is_alphabetic() => out.push(other),
            _ => {}
        }
        place += 1;
    }
    palatalize(&out)
}

/// The sound of a syllable's vowels.
#[allow(clippy::too_many_arguments)]
fn vowel(
    vowels: &[char],
    stressed: bool,
    after_stress: bool,
    last: bool,
    coda: Option<bool>,
    open: bool,
    letters: &[char],
    end: usize,
) -> String {
    let before_nasal = matches!(letters.get(end), Some('m' | 'n'))
        && (letters.get(end + 1).is_some_and(|n| is_vowel(*n)) || letters.get(end + 1) == Some(&'h'));
    if vowels.len() == 2 {
        let pair: String = vowels.iter().collect();
        let sound = match pair.as_str() {
            "ai" | "ái" => "I",
            "au" | "áu" => "W",
            "ei" | "éi" => "A",
            "eu" => "eʊ",
            "éu" => "ɛʊ",
            "ou" => "ow",
            "oi" | "ôi" => "oɪ",
            "ói" => "ɔɪ",
            "ui" => "uɪ",
            "iu" => "iʊ",
            "ão" => "ɐ̃ʊ̃",
            "ãe" => "ɐ̃ɪ̃",
            "õe" => "õɪ̃",
            _ => "",
        };
        if !sound.is_empty() {
            return sound.to_owned();
        }
    }
    let mut out = String::new();
    for (index, letter) in vowels.iter().enumerate() {
        let with_coda = index == vowels.len() - 1 && coda.is_some();
        let final_vowel = last && index == vowels.len() - 1 && coda.is_none();
        let sound = if with_coda {
            let lips = coda == Some(true);
            let at_end = end + 1 == letters.len();
            match base(*letter) {
                'a' if at_end && letters[end] == 'm' => "ɐ̃ʊ̃".to_owned(),
                'a' => if lips { "ɐ̃m" } else { "ɐ̃ŋ" }.to_owned(),
                'e' => if lips { "Am" } else { "Aŋ" }.to_owned(),
                'i' => if lips { "im" } else { "iŋ" }.to_owned(),
                'o' => if lips { "om" } else { "oŋ" }.to_owned(),
                _ => if lips { "ũm" } else { "ũŋ" }.to_owned(),
            }
        } else {
            match *letter {
                'á' | 'à' => "a".to_owned(),
                'â' => "ɐ".to_owned(),
                'ã' => "ɐ̃".to_owned(),
                'é' => "ɛ".to_owned(),
                'ê' => "e".to_owned(),
                'í' | 'y' => "i".to_owned(),
                'ó' => "ɔ".to_owned(),
                'ô' => "o".to_owned(),
                'õ' => "õ".to_owned(),
                'ú' if before_nasal => "ũ".to_owned(),
                'ú' => "u".to_owned(),
                'a' if before_nasal && (stressed || letters.get(end + 1) == Some(&'h')) => "ɐ̃".to_owned(),
                'a' if !stressed && (final_vowel || before_nasal) => "æ".to_owned(),
                'a' => "a".to_owned(),
                'e' if stressed && open => "ɛ".to_owned(),
                'e' if final_vowel && !stressed => "y".to_owned(),
                'e' => "e".to_owned(),
                'o' if stressed && open => "ɔ".to_owned(),
                'o' if !stressed && (final_vowel || after_stress) => "ʊ".to_owned(),
                'o' => "o".to_owned(),
                'i' | 'u' => letter.to_string(),
                other => other.to_string(),
            }
        };
        out.push_str(&sound);
    }
    out
}

/// `t` and `d` before an `i` sound become `ʧ` and `ʤ`.
fn palatalize(phonemes: &str) -> String {
    let chars: Vec<char> = phonemes.chars().collect();
    let mut out = String::with_capacity(phonemes.len());
    for (place, sound) in chars.iter().enumerate() {
        let next = chars[place + 1..].iter().find(|c| !matches!(c, 'ˈ' | 'ˌ')).copied();
        let front = next.is_some_and(|n| matches!(n, 'i' | 'y' | 'j' | 'ɪ'));
        match sound {
            't' if front => out.push('ʧ'),
            'd' if front => out.push('ʤ'),
            other => out.push(*other),
        }
    }
    out
}

/// Letters with their accents as one character, however the text wrote them.
fn composed(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for letter in word.chars() {
        let joined = match (out.chars().last(), letter) {
            (Some(base), '\u{301}') => "aeiou".find(base).map(|at| ['á', 'é', 'í', 'ó', 'ú'][at]),
            (Some('a'), '\u{300}') => Some('à'),
            (Some(base), '\u{302}') => "aeo".find(base).map(|at| ['â', 'ê', 'ô'][at]),
            (Some(base), '\u{303}') => "ao".find(base).map(|at| ['ã', 'õ'][at]),
            (Some('c'), '\u{327}') => Some('ç'),
            (Some('u'), '\u{308}') => Some('ü'),
            _ => None,
        };
        match joined {
            Some(joined) => {
                out.pop();
                out.push(joined);
            }
            None => out.push(letter),
        }
    }
    out
}

/// A word as said alone: the list first, then the rules.
fn unicode_mark(letter: char) -> bool {
    matches!(letter, '\u{300}'..='\u{36f}')
}

/// A word, or two joined by a hyphen: a pronoun after it belongs to the verb,
/// "encontrá-la", anything else is a word of its own, "segunda-feira".
fn hyphenated(written: &str) -> String {
    const CLITICS: &[&str] =
        &["la", "las", "lo", "los", "lhe", "lhes", "me", "te", "se", "nos", "vos", "o", "a", "os", "as"];
    let parts: Vec<&str> = written.split('-').filter(|part| !part.is_empty()).collect();
    let mut words: Vec<String> = Vec::new();
    for part in parts {
        let lower = part.to_lowercase();
        match words.last_mut() {
            Some(last) if CLITICS.contains(&lower.as_str()) => last.push_str(part),
            _ => words.push(part.to_owned()),
        }
    }
    words.iter().map(|part| word(part)).collect::<Vec<_>>().join(" ")
}

fn word(word: &str) -> String {
    let lower = composed(&word.to_lowercase().replace('-', ""));
    SPOKEN
        .iter()
        .find(|(written, _)| *written == lower)
        .map_or_else(|| word_phonemes(&lower), |(_, said)| (*said).to_owned())
}

fn starts_with_vowel(phonemes: &str) -> bool {
    phonemes.chars().find(|c| !matches!(c, 'ˈ' | 'ˌ')).is_some_and(|c| {
        matches!(
            c,
            'a' | 'e' | 'i' | 'o' | 'u' | 'ɛ' | 'ɔ' | 'ɐ' | 'æ' | 'ʊ' | 'A' | 'I' | 'W' | 'O' | 'y' | 'ũ'
        )
    })
}

fn starts_voiced(phonemes: &str) -> bool {
    starts_with_vowel(phonemes)
        || phonemes.chars().find(|c| !matches!(c, 'ˈ' | 'ˌ')).is_some_and(|c| {
            matches!(
                c,
                'b' | 'd' | 'ɡ' | 'l' | 'm' | 'n' | 'ɲ' | 'v' | 'z' | 'ʒ' | 'ʤ' | 'x' | 'r' | 'ɾ' | 'w' | 'j'
            )
        })
}

/// `text` in Brazilian Portuguese as phonemes for Kokoro. Numbers are left out: write
/// them in words before.
pub fn phonemes_pt_br(text: &str) -> String {
    // Words and what lies between them, which keeps the punctuation the model knows.
    let mut pieces: Vec<(bool, String)> = Vec::new();
    let mut current = String::new();
    for letter in text.chars() {
        if letter.is_alphabetic() || unicode_mark(letter) || (letter == '-' && !current.is_empty()) {
            current.push(letter);
            continue;
        }
        if !current.is_empty() {
            pieces.push((true, hyphenated(&current)));
            current.clear();
        }
        let kept = match letter {
            '.' | ',' | '!' | '?' | ':' | ';' | '…' | '—' | '(' | ')' | '"' | '“' | '”' => {
                Some(letter)
            }
            ' ' | '\n' | '\t' => Some(' '),
            _ => None,
        };
        if let Some(kept) = kept {
            pieces.push((false, kept.to_string()));
        }
    }
    if !current.is_empty() {
        pieces.push((true, hyphenated(&current)));
    }
    // How a word ends depends on how the next begins, when nothing pauses between them.
    for index in 0..pieces.len() {
        if !pieces[index].0 {
            continue;
        }
        let next = pieces[index + 1..]
            .iter()
            .take_while(|(is_word, text)| *is_word || text == " ")
            .find(|(is_word, _)| *is_word)
            .map(|(_, text)| text.clone());
        let Some(next) = next else { continue };
        let said = &mut pieces[index].1;
        let reducible = |said: &str, last: char| {
            // Only a vowel that ends the word after a consonant: not "saiu" or "painel".
            let mut back = said.chars().rev();
            back.next() == Some(last) && back.next().is_some_and(|c| !"aeiouɛɔɐæAIWOˈˌ\u{303}".contains(c))
        };
        if starts_with_vowel(&next) {
            if reducible(said, 'y') && !matches!(said.as_str(), "ky" | "ˌely") {
                said.pop();
                let before = said.chars().last();
                if matches!(before, Some('r' | 'ɾ' | 'l')) {
                    said.push('i');
                } else if next.starts_with("ˈo") || next.starts_with("ˈɔ") {
                    said.push('y');
                } else {
                    said.push('j');
                }
            } else if reducible(said, 'ʊ') && (said.contains('ˈ') || said.contains('ˌ')) && said != "kˌomʊ"
            {
                said.pop();
                said.push('w');
            } else if said.ends_with('r') {
                said.pop();
                said.push('ɾ');
            }
        }
        if said.ends_with('s') && starts_voiced(&next) {
            said.pop();
            said.push('z');
        }
    }
    // A small word that ends the sentence is said in full.
    for index in 0..pieces.len() {
        let ends = pieces.get(index + 1).is_none_or(|(is_word, text)| !is_word && text != " ");
        let only_secondary = !pieces[index].1.contains('ˈ');
        if pieces[index].0 && ends && only_secondary && pieces[index].1.contains('ˌ') {
            pieces[index].1 = pieces[index].1.replacen('ˌ', "ˈ", 1);
        }
        if pieces[index].0
            && ends
            && only_secondary
            && let Some(rest) = pieces[index].1.strip_prefix("dˌ")
        {
            pieces[index].1 = format!("dˈ{rest}");
        }
    }
    let mut out = String::new();
    for (_, piece) in pieces {
        if piece == " " && out.ends_with(' ') {
            continue;
        }
        out.push_str(&piece);
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_follows_the_spelling() {
        assert_eq!(word("casa"), "kˈazæ");
        assert_eq!(word("valor"), "valˈor");
        assert_eq!(word("próximo"), "prˈɔsimʊ");
        assert_eq!(word("dia"), "ʤˈiæ");
        assert_eq!(word("lançamentos"), "lˌɐ̃ŋsæmˈAŋtʊs");
    }

    #[test]
    fn the_sounds_are_brazilian() {
        assert_eq!(word("tipo"), "ʧˈipʊ");
        assert_eq!(word("saldo"), "sˈWdʊ");
        assert_eq!(word("botão"), "botˈɐ̃ʊ̃");
        assert_eq!(word("linha"), "lˈiɲæ");
        assert_eq!(word("regra"), "xˈɛɡræ");
    }

    #[test]
    fn words_change_before_the_next() {
        assert_eq!(phonemes_pt_br("Clique em Agenda."), "klˈikj ˈAŋ ˌaʒˈAŋdæ.");
        assert_eq!(word("saiu"), "saˈiʊ");
        assert_eq!(word("vídeo"), "vˈiʤjʊ");
        assert_eq!(word("salário"), "sˌalˈaɾjʊ");
        assert_eq!(word("junta"), "ʒˈu\u{303}ŋtæ");
        assert_eq!(word("Ca\u{301}lculo"), word("Cálculo"));
        assert_eq!(phonemes_pt_br("as contas do banco"), "as kˈoŋtæz dʊ bˈɐ̃ŋkʊ");
    }

    /// Compares with what espeak-ng writes for the same sentences, kept outside the
    /// repository: `RAVENRS_PHONEMES=file.json cargo test -p ravenrs-ai phonemes_close -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs sentences written by espeak-ng"]
    fn phonemes_close_to_espeak() {
        let file = std::env::var("RAVENRS_PHONEMES").expect("RAVENRS_PHONEMES");
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(file).expect("file")).expect("json");
        let (mut distance, mut length, mut same) = (0, 0, 0);
        let mut worst = Vec::new();
        for case in cases.as_array().expect("cases") {
            let text = case["text"].as_str().expect("text");
            let wanted = case["phonemes"].as_str().expect("phonemes");
            let ours = phonemes_pt_br(text);
            let words_wanted: Vec<&str> = wanted.split(' ').collect();
            let words_ours: Vec<&str> = ours.split(' ').collect();
            for (ours, wanted) in words_ours.iter().zip(&words_wanted) {
                if ours == wanted {
                    same += 1;
                } else {
                    worst.push(format!("{wanted} ≠ {ours}"));
                }
            }
            let ours: Vec<char> = ours.chars().collect();
            let wanted: Vec<char> = wanted.chars().collect();
            distance += levenshtein(&ours, &wanted);
            length += wanted.len();
        }
        let words: usize = cases
            .as_array()
            .expect("cases")
            .iter()
            .map(|c| c["phonemes"].as_str().expect("p").split(' ').count())
            .sum();
        worst.sort();
        worst.dedup();
        for line in worst.iter().take(400) {
            eprintln!("{line}");
        }
        eprintln!(
            "{:.1}% of the characters match, {same} of {words} words the same",
            100.0 * (1.0 - distance as f32 / length as f32)
        );
    }

    fn levenshtein(a: &[char], b: &[char]) -> usize {
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, x) in a.iter().enumerate() {
            let mut previous = row[0];
            row[0] = i + 1;
            for (j, y) in b.iter().enumerate() {
                let keep = row[j + 1];
                row[j + 1] = (previous + usize::from(x != y)).min(row[j] + 1).min(row[j + 1] + 1);
                previous = keep;
            }
        }
        row[b.len()]
    }
}
