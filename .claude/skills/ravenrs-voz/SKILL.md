---
name: ravenrs-voz
description: Roteiro da voz local do RavenRS - Kokoro em Rust no ravenrs-ai, regras de fonema pt-BR próprias, qual voz cada uso leva, como diagnosticar uma fala errada (fonema antes do modelo), como gravar e regravar a narração dos filmes e o que a IA não consegue conferir sozinha. Use sempre que o pedido citar voz, fala, narração, locução, Kokoro, fonema, pronúncia, pf_dora, pm_alex, pm_santa, SpeechEngine, LocalVoice, VoiceManager, narracao.voz, ou quando o Alex disser que ouviu algo errado num filme ou vídeo, mesmo que não cite esta skill.
---

# A voz local do RavenRS

A voz roda na máquina de quem usa, sem serviço pago (memória `ia-sempre-local`). O motor é o Kokoro 82M, portado para Rust no `crates/ravenrs-ai` (27.1.18, PR #99). O README do crate explica cada etapa; esta skill diz como trabalhar com ela.

## Onde está cada parte

| Parte | Arquivo |
|---|---|
| Texto vira fonemas pt-BR, com regras nossas (o espeak-ng é GPL e não entra) | `crates/ravenrs-ai/src/phonemes.rs` (`phonemes_pt_br`) |
| Fonemas viram som, etapa por etapa como o código publicado em PyTorch | `crates/ravenrs-ai/src/kokoro.rs`, `src/spectrum.rs` |
| A porta pública: `SpeechEngine` (bloqueia), `LocalVoice` (thread própria), `VoiceManager` (tela) | `crates/ravenrs-ai/src/speech.rs` |
| Números, dinheiro e datas por extenso antes de falar | `examples/family-finance/src/ui/spoken.rs` |
| Narração gravada dos filmes do Raven Finanças | `examples/family-finance/assets/treinamentos/narracao.voz` |
| Como ela é gravada e empacotada | `record_narration` e `pack_narration` em `examples/family-finance/tests/suite/training.rs` |
| Vídeos do canal | `tools/raven-channel-video/src/main.rs` |

## Qual voz em cada uso

- **Filmes de treinamento:** `pm_santa`, que o Alex achou mais legal e engraçada. É o padrão do `record_narration`.
- **Vídeos do canal e de marketing:** a pessoa escolhe entre `pf_dora`, `pm_alex` e `pm_santa`. A `pf_dora` foi aprovada por ele em 07/11 para o canal.
- **App ao vivo:** o que o `VoiceManager` guardar em `voice-settings.json`.

O Kokoro só fala português do Brasil. O LFM3 Audio fala as outras 19 línguas, mas o Alex achou a locução dele ruim: não volte para ele sem pedido.

## Quando o Alex ouve algo errado

A IA não ouve. Quem ouve é ele, e o primeiro suspeito é sempre o fonema, não o modelo: até hoje, toda palavra errada foi uma regra de fonema faltando (o "vidÊo" de 07/11 era isso).

1. **Compare os fonemas da mesma frase** com o que o espeak-ng escreve. Gere o gabarito no scratchpad, nunca no repositório: um JSON `[{"text": "...", "phonemes": "..."}]` com a saída do `espeak-ng -v pt-br -q --ipa` (instalado só para isso, no scratchpad) e rode:

   ```bash
   RAVENRS_PHONEMES=<scratchpad>/gabarito.json cargo test --manifest-path camadas/automacao/Cargo.toml -p ravenrs-ai phonemes_close -- --ignored --nocapture
   ```

   O teste lista as palavras diferentes (`esperado ≠ nosso`). A correção entra em `phonemes.rs`, com um caso novo no teste de palavras do mesmo arquivo.
2. **Ouça o modelo sem as regras**, se os fonemas já batem: `RAVENRS_SAY_PHONEMES` manda os fonemas como estão. Os comandos completos estão na seção "Checking by ear" do README do crate.
3. **Só então desconfie do modelo.** A régua é o próprio PyTorch com outra semente: ele não reproduz bit a bit, e duas sementes do código publicado diferem tanto quanto o nosso difere dele. `kokoro_matches_the_published_code` compara cada etapa com números escritos por ele.

Dígitos saem calados: escreva os números por extenso (#96). Troca de voz ou de modelo não resolve fonema errado.

## Gravar e regravar a narração dos filmes

A narração é gravada antes, para quem assiste não baixar modelo nenhum. Um texto é achado pelas palavras exatas: texto que mudou sem regravar volta à voz do sistema.

```bash
# Tudo, com pm_santa (em release):
FF_AI_MODELS=<pasta dos modelos> cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::record_narration -- --ignored --nocapture
# Só o que o narracao.voz ainda não diz, acrescentado a ele:
FF_NARRATION_ADD=1 FF_AI_MODELS=<pasta> cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::record_narration -- --ignored --nocapture
# Seis tomadas das falas 12 e 40 em WAV, para o Alex escolher de ouvido, e depois empacotar:
FF_NARRATION_TAKES=12,40 FF_NARRATION_WAVS=<scratchpad>/wavs FF_AI_MODELS=<pasta> cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::record_narration -- --ignored
FF_NARRATION_WAVS=<scratchpad>/wavs cargo test --release --manifest-path examples/family-finance/Cargo.toml --test suite training::pack_narration -- --ignored
```

- Frase curta como "Clique em…" sai mal: grave tomadas extras dessas e deixe o Alex escolher.
- `FF_NARRATION_SPEAKER` troca a voz e `FF_NARRATION_MODEL` o modelo. Não troque sem pedido.
- Os WAV vão para o scratchpad e saem na limpeza do fim da tarefa.

## O que a IA não prova sozinha

- **Que soa bem.** Confira toda fala nova com um reconhecedor independente (Whisper, pelo `Transcriber`, ou o do sistema) e diga ao Alex que a conferência foi por transcrição, não de ouvido.
- **Linux e Windows.** Só foi medido no macOS (#98). Não compile para esses sistemas no Mac (memória `linux-windows-so-o-alex-testa`).
- **Microfone.** O `Transcriber` existe, mas o RavenRS não captura o microfone ainda: não há conversa por voz.

## Pendências

- #96: números em dígitos.
- #97: a voz no `channel-video`.
- #98: Linux e Windows.
