#!/usr/bin/env python3
"""Build the RavenDevLab lesson from verified RavenRS screens and source code.

Requires macOS `say` and requirements.txt. No screen capture or external service.
"""

import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import textwrap
import wave
import zipfile

import imageio_ffmpeg
from PIL import Image, ImageDraw, ImageFont
from pygments import lex
from pygments.lexers import RustLexer, TOMLLexer
from pygments.token import Token

HERE = Path(__file__).resolve().parent
OUT = HERE / "output"
ROOT = HERE.parents[2]
FFMPEG = imageio_ffmpeg.get_ffmpeg_exe()
VOICE = os.environ.get("RAVEN_VIDEO_VOICE", "Luciana (Português (Brasil))")
W, H = 1920, 1080
BG, PANEL = "#0b1020", "#141c30"
WHITE, MUTED, ACCENT = "#edf1ff", "#a3b1ce", "#8b9bff"
SANS = "/System/Library/Fonts/Supplemental/Arial.ttf"
BOLD = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
MONO = "/System/Library/Fonts/Menlo.ttc"

SOURCE = (HERE / "projeto/src/main.rs").read_text()
ENTITY = SOURCE.split("fn main()")[0].strip()
MAIN = "fn main()" + SOURCE.split("fn main()", 1)[1]
MANIFEST = '''[package]
name = "minhas-tarefas"
version = "0.1.0"
edition = "2024"

[dependencies]
ravenrs = "=27.1.14"
ravenrs-app = "=27.1.14"
ravenrs-app-ui = "=27.1.14"'''

# The narration and the visual are a single editable source of truth.
SCENES = [
    dict(title="Seu primeiro app com RavenRS", kind="hero", tag="RUST + VS CODE",
         text="Bem-vindo ao Raven Dev Lab. Nesta aula, você vai criar um cadastro de tarefas com Rust e Raven RS, usando o VS Code. Vamos sair de uma struct e chegar a uma aplicação com lista, formulário e validação."),
    dict(title="O resultado que vamos construir", kind="app", image="app-4.png", tag="APP REAL · RAVENRS",
         text="Este é o resultado: Minhas tarefas. A navegação, a tabela e os comandos são gerados pelo framework. Você declara os campos do registro e personaliza os rótulos. Vamos construir esse exemplo passo a passo."),
    dict(title="Prepare o ambiente", kind="cards", tag="ANTES DE COMEÇAR",
         items=[("Rust + Cargo", "Rust 1.88 ou superior · ferramentas nativas"), ("Visual Studio Code", "Extensão: rust-lang.rust-analyzer"), ("Confira no terminal", "rustc --version    |    cargo --version")],
         text="Você precisa do Rust, do Cargo e das ferramentas de compilação da sua plataforma. Use Rust um ponto oitenta e oito ou superior. No VS Code, instale a extensão rust-analyzer. Abra um terminal novo e confira as versões com estes dois comandos."),
    dict(title="Crie o projeto e abra no VS Code", kind="terminal", tag="01 / PROJETO",
         code="cargo new minhas-tarefas\ncd minhas-tarefas\ncode .", note="Sem o comando code? Use Arquivo → Abrir Pasta.",
         text="No terminal, execute cargo new minhas-tarefas. Entre na pasta criada e execute code ponto para abrir o projeto no VS Code. Se o comando code não estiver disponível, abra a pasta pelo menu Arquivo do editor."),
    dict(title="Adicione as três bibliotecas", kind="terminal", tag="02 / DEPENDÊNCIAS",
         code="cargo add ravenrs@=27.1.14 ravenrs-app@=27.1.14 ravenrs-app-ui@=27.1.14", note="Execute o comando completo no terminal integrado.",
         text="No VS Code, abra Terminal, Novo Terminal. Execute cargo add com estas três bibliotecas e versões exatas. O comando completo está no material da aula. Raven RS executa a interface. Raven RS app descreve o modelo. E Raven RS app UI gera as telas."),
    dict(title="Confira o Cargo.toml", kind="editor", file="Cargo.toml", code=MANIFEST, language="toml", tag="VERSÕES FIXADAS",
         text="O Cargo atualiza o arquivo Cargo ponto toml. O sinal de igual fixa a versão vinte e sete, um, quatorze. Isso ajuda a reproduzir a aula, porque a API do framework ainda pode mudar entre versões."),
    dict(title="Declare a entidade Task", kind="editor", file="src/main.rs", code=ENTITY, highlight=[4, 5, 6], tag="03 / MODELO",
         text="Agora substitua o conteúdo de source, main ponto érre ésse pelo código da aula. Comece pelos imports e pela struct Task. O derive Business Object descreve a entidade. O atributo object define Tarefas na navegação, dentro do grupo Organização."),
    dict(title="Campos que descrevem a interface", kind="editor", file="src/main.rs", code=ENTITY, highlight=[7, 8, 9, 10, 11, 12], tag="CHAVE · TÍTULO · CONCLUSÃO",
         text="O campo id é a chave do registro. O título é uma String. Caption define o rótulo na tela. Default property identifica o texto que representa a tarefa, e required torna esse campo obrigatório. O booleano done gera o controle de Concluída no formulário."),
    dict(title="Registre a entidade no App", kind="editor", file="src/main.rs", code=MAIN, highlight=[2, 3, 4, 5], tag="04 / APLICAÇÃO",
         text="Na função main, crie um App com o título Minhas tarefas. Selecione o português brasileiro, registre a entidade Task e chame into view. É essa composição que gera a navegação, a lista e o formulário a partir do modelo."),
    dict(title="Abra a janela nativa", kind="editor", file="src/main.rs", code=MAIN, highlight=[7, 8, 9, 10], tag="05 / JANELA",
         text="Por fim, passe essa interface para native run. Window Options define o título e o tamanho inicial da janela: mil e duzentos por setecentos e sessenta. A função retorna o resultado da execução. O código completo está disponível junto com este vídeo."),
    dict(title="Execute pelo terminal integrado", kind="terminal", tag="06 / EXECUÇÃO", code="cargo run", note="A primeira compilação baixa e compila as dependências.",
         text="Salve o arquivo e execute cargo run no terminal integrado. A primeira compilação pode levar alguns minutos. Quando ela terminar, a janela do aplicativo será aberta. Não é necessário desenhar cada botão e cada campo manualmente."),
    dict(title="A lista está pronta", kind="app", image="app-0.png", tag="07 / DEMONSTRAÇÃO REAL",
         text="Aqui está a lista vazia de tarefas. Observe o grupo Organização, a navegação Tarefas e o comando Novo. A tabela já apresenta as colunas dos campos declarados. Agora vamos criar o primeiro registro."),
    dict(title="Novo abre o formulário", kind="app", image="app-1.png", tag="FORMULÁRIO GERADO",
         text="Clique em Novo. O formulário mostra o campo Título e a opção Concluída. Os comandos de salvar e cancelar também foram gerados. O asterisco ao lado do título indica que o preenchimento é obrigatório."),
    dict(title="Teste a validação", kind="app", image="app-2.png", tag="REQUIRED EM AÇÃO",
         text="Sem preencher o título, clique em Salvar. O app apresenta a mensagem Título é obrigatório. Essa validação vem do atributo required que colocamos no modelo. Agora preencha o título para continuar."),
    dict(title="Preencha e salve a tarefa", kind="app", image="app-3.png", tag="CRIANDO UM REGISTRO",
         text="Digite Publicar meu primeiro app no campo Título. Deixe Concluída desmarcada e clique em Salvar e fechar. Assim, você confirma o registro e retorna à lista de tarefas."),
    dict(title="Sua primeira tarefa cadastrada", kind="app", image="app-4.png", tag="RESULTADO",
         text="Pronto. A tarefa aparece na tabela. Você acabou de criar um aplicativo de cadastro com navegação, formulário e validação, partindo de uma struct Rust. As telas mostradas aqui foram geradas pela execução real deste exemplo."),
    dict(title="O próximo passo: persistência", kind="cards", tag="DADOS DESTA AULA: EM MEMÓRIA",
         items=[("Fechar o app apaga os registros", "Este primeiro exemplo usa armazenamento em memória."), ("Próxima evolução", "Adicionar ravenrs-app-sqlite para persistir os dados."), ("Experimente agora", "Crie outra tarefa e marque Concluída no formulário.")],
         text="Neste primeiro exemplo, os dados ficam em memória. Fechar o app apaga os registros. Para guardar as tarefas entre execuções, o próximo passo é adicionar a integração com SQLite. Antes disso, experimente criar outra tarefa e marcar a opção Concluída."),
    dict(title="Continue construindo com RavenRS", kind="end", tag="RAVEN DEV LAB",
         text="O projeto completo e os comandos estão no material que acompanha esta aula. Abra no VS Code, execute e experimente modificar os campos. Inscreva-se no Raven Dev Lab para acompanhar os próximos exemplos com Raven RS. Até a próxima aula."),
]


def run(args):
    subprocess.run([str(x) for x in args], check=True, stdout=subprocess.DEVNULL)


def font(size, mono=False, bold=False):
    return ImageFont.truetype(MONO if mono else (BOLD if bold else SANS), size)


def wrapped(draw, text, xy, size=34, width=1500, color=WHITE, bold=False, gap=12):
    f = font(size, bold=bold)
    x, y = xy
    line = ""
    for word in text.split():
        candidate = (line + " " + word).strip()
        if draw.textlength(candidate, font=f) > width and line:
            draw.text((x, y), line, font=f, fill=color)
            y += size + gap
            line = word
        else:
            line = candidate
    draw.text((x, y), line, font=f, fill=color)
    return y + size + gap


def base(scene, index):
    im = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(im)
    # Restrained studio frame; leaves a clean margin for subtitles.
    d.rectangle((0, 0, W, 8), fill=ACCENT)
    d.text((72, 35), "RAVEN DEV LAB", font=font(25, bold=True), fill=WHITE)
    d.text((W - 415, 37), "RavenRS 27.1.14  /  Aula 01", font=font(22), fill=MUTED)
    d.text((72, 102), scene["tag"], font=font(20, bold=True), fill=ACCENT)
    d.text((72, 142), scene["title"], font=font(48, bold=True), fill=WHITE)
    d.line((72, 218, W - 72, 218), fill="#26324b", width=2)
    d.text((72, 1018), "youtube.com/@RavenDevLab", font=font(22), fill=MUTED)
    d.text((W - 185, 1018), f"{index + 1:02d} / {len(SCENES):02d}", font=font(22, mono=True), fill=MUTED)
    d.rectangle((0, H - 5, int(W * (index + 1) / len(SCENES)), H), fill=ACCENT)
    return im, d


def syntax_color(token):
    if token in Token.Comment:
        return "#8c9bad"
    if token in Token.Literal.String:
        return "#e0b386"
    if token in Token.Keyword:
        return "#d7a4ee"
    if token in Token.Name.Function:
        return "#e8dca4"
    if token in Token.Name.Class or token in Token.Name.Builtin:
        return "#7bd7cb"
    if token in Token.Literal.Number:
        return "#a7d4a1"
    return "#d6e0f1"


def render_editor(im, d, scene):
    x, y, right, bottom = 72, 260, 1848, 964
    d.rounded_rectangle((x, y, right, bottom), radius=16, fill="#1e1e1e", outline="#39445b", width=2)
    d.rectangle((x + 1, y + 18, right - 1, y + 50), fill="#292b30")
    d.text((x + 25, y + 12), "minhas-tarefas — Visual Studio Code", font=font(20), fill="#c9d1e0")
    d.rectangle((x + 1, y + 51, x + 255, bottom - 26), fill="#252526")
    d.text((x + 24, y + 75), "EXPLORADOR", font=font(17, bold=True), fill=MUTED)
    d.text((x + 24, y + 123), "MINHAS-TAREFAS", font=font(18, bold=True), fill=WHITE)
    d.text((x + 38, y + 165), "⌄ src", font=font(20), fill=WHITE)
    d.text((x + 63, y + 204), "main.rs", font=font(20), fill=ACCENT)
    d.text((x + 38, y + 246), "Cargo.toml", font=font(20), fill=WHITE)
    cx, cy = x + 279, y + 70
    d.text((cx + 14, cy), scene["file"], font=font(22), fill=WHITE)
    d.line((cx, cy + 39, right - 15, cy + 39), fill="#38405a", width=1)
    top = cy + 65
    size, line_h = 25, 35
    f = font(size, mono=True)
    for number, line in enumerate(scene["code"].splitlines(), 1):
        yy = top + (number - 1) * line_h
        if yy + line_h > bottom - 40:
            raise ValueError("Code overflows the editor")
        if number in scene.get("highlight", []):
            d.rectangle((cx + 2, yy - 3, right - 18, yy + line_h - 2), fill="#30324b")
            d.rectangle((cx + 2, yy - 3, cx + 5, yy + line_h - 2), fill=ACCENT)
        d.text((cx + 16, yy), str(number).rjust(2), font=f, fill="#768091")
        xx = cx + 74
        lexer = TOMLLexer() if scene.get("language") == "toml" else RustLexer()
        for token, value in lex(line, lexer):
            value = value.rstrip("\n")
            if xx + d.textlength(value, font=f) > right - 20:
                raise ValueError(f"Code line too wide: {line}")
            d.text((xx, yy), value, font=f, fill=syntax_color(token))
            xx += d.textlength(value, font=f)
    d.rectangle((x + 1, bottom - 27, right - 1, bottom - 1), fill="#244878")
    d.text((x + 22, bottom - 25), "VISUALIZAÇÃO DIDÁTICA DO EDITOR  ·  CÓDIGO DO PROJETO", font=font(17), fill=WHITE)


def render(scene, index):
    im, d = base(scene, index)
    kind = scene["kind"]
    if kind == "editor":
        render_editor(im, d, scene)
    elif kind == "terminal":
        d.rounded_rectangle((100, 300, 1820, 865), radius=24, fill="#111722", outline="#36405c", width=2)
        d.text((142, 334), "TERMINAL INTEGRADO  /  VS CODE", font=font(22, bold=True), fill=MUTED)
        d.line((142, 386, 1778, 386), fill="#2a354b", width=2)
        terminal_size = 30 if max(map(len, scene["code"].splitlines())) > 60 else 39
        for i, line in enumerate(scene["code"].splitlines()):
            d.text((148, 440 + 66 * i), ("$ " if i == 0 else "  ") + line,
                   font=font(terminal_size, mono=True), fill="#bbd5ff")
        wrapped(d, scene["note"], (142, 766), size=27, color=MUTED)
    elif kind == "app":
        screen = Image.open(OUT / scene["image"]).convert("RGB")
        # Fit the actual 1200 x 760 render. Keep all controls visible.
        screen = screen.resize((1092, 692), Image.Resampling.LANCZOS)
        im.paste(screen, (414, 265))
        d.rounded_rectangle((412, 263, 1508, 959), radius=4, outline="#45516e", width=2)
        d.text((72, 305), "RavenRS", font=font(33, bold=True), fill=WHITE)
        wrapped(d, "Interface real gerada pelo exemplo", (72, 361), size=25, width=285, color=MUTED)
        d.text((72, 900), "1200 × 760", font=font(23, mono=True), fill=ACCENT)
    elif kind == "cards":
        for i, (heading, description) in enumerate(scene["items"]):
            yy = 286 + i * 220
            d.rounded_rectangle((100, yy, 1820, yy + 178), radius=20, fill=PANEL)
            d.text((138, yy + 28), f"0{i + 1}", font=font(36, mono=True), fill=ACCENT)
            d.text((238, yy + 28), heading, font=font(36, bold=True), fill=WHITE)
            wrapped(d, description, (238, yy + 89), size=29, width=1500, color=MUTED)
    else:
        d.text((112, 302), "UMA STRUCT.", font=font(87, bold=True), fill=WHITE)
        d.text((112, 407), "UM APP COMPLETO.", font=font(87, bold=True), fill=ACCENT)
        d.text((117, 552), "Rust  →  Modelo  →  Interface gerada", font=font(38), fill=MUTED)
        labels = ["Navegação", "Lista", "Formulário", "Validação"]
        for i, label in enumerate(labels):
            xx = 116 + i * 425
            d.rounded_rectangle((xx, 661, xx + 385, 761), radius=18, fill=PANEL)
            d.text((xx + 27, 691), label, font=font(32, bold=True), fill=WHITE)
        if kind == "end":
            d.text((117, 850), "Código e comandos no material da aula", font=font(36), fill=WHITE)
        else:
            d.text((117, 850), "Seu primeiro projeto com ravenrs-app no VS Code", font=font(36), fill=WHITE)
    return im


def stamp(seconds, srt=False):
    ms = round(seconds * 1000)
    hours, ms = divmod(ms, 3600000)
    minutes, ms = divmod(ms, 60000)
    secs, ms = divmod(ms, 1000)
    return f"{hours:02}:{minutes:02}:{secs:02},{ms:03}" if srt else f"{hours * 60 + minutes:02}:{secs:02}"


def main():
    OUT.mkdir(exist_ok=True)
    if not shutil.which("say"):
        raise SystemExit("A narração deste roteiro requer macOS com o comando say.")
    for i in range(5):
        if not (OUT / f"app-{i}.png").exists():
            raise SystemExit("Gere primeiro as telas reais: veja README.md.")
    clips, subtitles, timeline = [], [], []
    elapsed = 0.0
    for i, scene in enumerate(SCENES):
        stem = OUT / f"scene-{i:02}"
        image_path = stem.with_suffix(".png")
        render(scene, i).save(image_path)
        script_path = stem.with_suffix(".txt")
        script_path.write_text(scene["text"])
        aiff, wav = stem.with_suffix(".aiff"), stem.with_suffix(".wav")
        if not aiff.exists() or aiff.stat().st_mtime < script_path.stat().st_mtime:
            run(["say", "-v", VOICE, "-r", "165", "-f", script_path, "-o", aiff])
        run([FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-i", aiff,
             "-ar", "48000", "-ac", "1", wav])
        with wave.open(str(wav)) as audio:
            speech_duration = audio.getnframes() / audio.getframerate()
        # Snap to video frames for exact chapters across concatenated clips.
        frames = math.ceil((speech_duration + 1.2) * 30)
        duration = frames / 30
        clip = stem.with_suffix(".mp4")
        run([FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-loop", "1", "-framerate", "30",
             "-i", image_path, "-i", wav, "-vf", "fade=t=in:st=0:d=0.2,format=yuv420p",
             "-af", "adelay=250:all=1,apad", "-t", f"{duration:.6f}",
             "-c:v", "libx264", "-preset", "veryfast", "-tune", "stillimage", "-crf", "19",
             "-c:a", "aac", "-b:a", "192k", "-ar", "48000", "-ac", "2",
             "-video_track_timescale", "30000", clip])
        sentences = re.split(r"(?<=[.!?])\s+", scene["text"])
        weight = sum(len(s.split()) for s in sentences)
        sub_at = elapsed + 0.25
        for sentence in sentences:
            span = speech_duration * len(sentence.split()) / weight
            lines = textwrap.wrap(sentence, width=65)
            # For long sentences, split the estimated speaking interval into readable cues.
            groups = [lines[n:n + 2] for n in range(0, len(lines), 2)]
            group_weight = sum(len(" ".join(group)) for group in groups)
            for group in groups:
                cue_span = span * len(" ".join(group)) / group_weight
                subtitles.append((sub_at, sub_at + cue_span, "\n".join(group)))
                sub_at += cue_span
        timeline.append(dict(title=scene["title"], start=round(elapsed, 3), duration=round(duration, 3),
                             speech_duration=round(speech_duration, 3), kind=scene["kind"]))
        clips.append(clip)
        elapsed += duration
        print(f"{i + 1}/{len(SCENES)} · {stamp(elapsed)} · {scene['title']}", flush=True)
    concat = OUT / "segments.txt"
    concat.write_text("".join(f"file '{p.name}'\n" for p in clips))
    # Concat without a second lossy encode; all segments use identical encoding settings.
    final = OUT / "ravendevlab-primeiro-app.mp4"
    run([FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", concat,
         "-c", "copy", "-movflags", "+faststart", final])
    (OUT / "legendas.srt").write_text("\n\n".join(
        f"{i + 1}\n{stamp(start, True)} --> {stamp(end, True)}\n{text}"
        for i, (start, end, text) in enumerate(subtitles)) + "\n")
    (OUT / "timeline.json").write_text(json.dumps(timeline, ensure_ascii=False, indent=2))
    (OUT / "roteiro.md").write_text("# Roteiro — RavenDevLab\n\n" + "\n\n".join(
        f"## {stamp(t['start'])} — {s['title']}\n\nCena: {s['kind']}.\n\n{s['text']}"
        for t, s in zip(timeline, SCENES)))
    chapters = "\n".join(f"{stamp(t['start'])} {t['title']}" for t in timeline)
    (OUT / "youtube.md").write_text(
        "# Título\n\nSeu primeiro app Rust com RavenRS no VS Code | Cadastro de tarefas\n\n"
        "# Descrição\n\nAprenda a criar um app nativo com Rust, ravenrs-app e VS Code. "
        "Nesta aula, uma struct gera navegação, lista, formulário e validação de campos.\n\n"
        "Versão usada: RavenRS 27.1.14. Rust 1.88 ou superior. Dados em memória nesta aula.\n\n"
        "Instale Rust e as ferramentas nativas da sua plataforma. No VS Code, instale rust-analyzer.\n\n"
        "Comandos:\ncargo new minhas-tarefas\ncd minhas-tarefas\ncode .\n"
        "cargo add ravenrs@=27.1.14 ravenrs-app@=27.1.14 ravenrs-app-ui@=27.1.14\ncargo run\n\n"
        "Código do exemplo: https://github.com/alexkads/RavenRS\n"
        "O pacote local minhas-tarefas.zip acompanha a produção desta aula. "
        "Disponibilize esse arquivo ou publique o código do tutorial e acrescente o link antes de postar.\n\n"
        "Documentação: https://docs.rs/ravenrs-app/27.1.14/ravenrs_app/\n"
        "VS Code e Rust: https://code.visualstudio.com/docs/languages/rust\n\n"
        "Capítulos:\n" + chapters + "\n\n"
        "Produção: voz sintética em português brasileiro. As telas do app vêm de execução real; "
        "as cenas do editor são representações didáticas do VS Code.\n\n"
        "#Rust #RavenRS #VSCode #RavenDevLab\n")
    with zipfile.ZipFile(OUT / "minhas-tarefas.zip", "w", zipfile.ZIP_DEFLATED) as archive:
        for file in (HERE / "projeto").rglob("*"):
            if file.is_file() and "target" not in file.parts and file.name != ".DS_Store":
                archive.write(file, Path("minhas-tarefas") / file.relative_to(HERE / "projeto"))
    cover, d = base(SCENES[0], 0)
    d.rectangle((0, 80, W, H), fill=BG)
    d.text((96, 190), "SEU PRIMEIRO", font=font(100, bold=True), fill=WHITE)
    d.text((96, 300), "APP EM RUST", font=font(123, bold=True), fill=ACCENT)
    d.text((104, 475), "RavenRS + VS Code", font=font(61, bold=True), fill=WHITE)
    screen = Image.open(OUT / "app-4.png").convert("RGB")
    screen.thumbnail((980, 470), Image.Resampling.LANCZOS)
    cover.paste(screen, (850, 580))
    d.text((104, 725), "MODELO", font=font(43, bold=True), fill=WHITE)
    d.text((104, 792), "↓", font=font(50), fill=ACCENT)
    d.text((104, 860), "INTERFACE GERADA", font=font(43, bold=True), fill=WHITE)
    cover.resize((1280, 720), Image.Resampling.LANCZOS).save(OUT / "thumbnail.jpg", quality=95)
    print(f"Concluído: {final} ({stamp(elapsed)})", flush=True)


if __name__ == "__main__":
    main()
