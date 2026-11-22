import { atom, read, update } from 'claude-code'
import type { Hook, Register, RenderChildren } from 'claude-code'

import type { Block, Last, Report, Update } from '../types'

const PANE = 'raven-test-studio'
const TITLE = 'Raven Test Studio'
const TOOL = 'mcp__raven-test-studio__run_scenario'
const PROJECT_TOOL = 'mcp__raven-test-studio__scan_project'
// The scenarios that come with the Studio, by the name its `--run` takes.
const SCENARIOS = ['autoteste', 'financas'] as const
// What each scenario is, for the empty pane.
const ABOUT = {
  autoteste: {
    icon: '◆',
    title: 'O Studio testa o Studio',
    blurb: 'Abre outro Studio, roda o cenário dele, confere cada bloco, para, anda passo a passo e confere uma falha de propósito. 18 blocos.',
  },
  financas: {
    icon: '💰',
    title: 'Os cenários do Raven Finanças',
    blurb: 'os arquivos .raventest que o próprio app traz em tests/e2e, rodados nele aberto à parte, sem janela e nos dados de exemplo',
  },
} as const

/** The title of a scenario by the name `--run` takes. */
function titleOf(name: string): string {
  return name in ABOUT ? ABOUT[name as keyof typeof ABOUT].title : name
}
// Blocks a group shows before "mais N".
const SHOWN = 4
// The design's colors, as the person's theme names them: the brand's mark, a card's fill and
// edge, and the darker ground of the screen a failure shows.
const BRAND = 'claude'
const CARD = 'surface-2'
const LINE = 'subtle'
const DEEP = 'surface-0'

const last = atom(
  { plugin: 'raven-test-studio', key: 'last' } as const,
  { report: null, problem: null, asked: null, via: null, delegated: null } as Last,
  { shape: 'last-3' },
)
const running = atom({ plugin: 'raven-test-studio', key: 'running' } as const, false, { shape: 'running-1' })
// Whether there is a newer Studio, as `--atualizacao` answers, and how installing it goes.
const updates = atom(
  { plugin: 'raven-test-studio', key: 'update' } as const,
  { answer: null, installing: false, stage: null, done: null, failed: null } as Update,
  { shape: 'update-1' },
)
// How often the pane asks again, as the Studio's window does.
const CHECK_EVERY_MS = 3_600_000
// How often the pane asks the Studio only its version, to see at once that it was updated
// outside the pane (by its window or the command line). `--versao` reads nothing on the network.
const VERSION_EVERY_MS = 120_000

/** The engine interface every hook receives. */
type Ops = Parameters<Hook<'ui.render'>>[0]

/** How a run asks the Studio: a delegated one hands the scenario to another agent first. */
type Ask = { scenario: string; noCache: boolean; delegate: string | null; model: string | null }

/**
 * The Studio to run, and how the pane names it. In the RavenRS checkout it is the code there
 * (`cargo run`, built where Cargo builds: its `target` or the session's `CARGO_TARGET_DIR`), so
 * a change is tested as it is made; anywhere else it is the installed Studio, which updates
 * itself, found where its installer puts it, or by name in `PATH`.
 */
async function studio($: Ops): Promise<{ argv: string[]; via: string }> {
  const cwd = await $.session.cwd()
  if (await $.fs.exists(`${cwd}/tools/raven-test-studio/Cargo.toml`)) {
    return { argv: ['cargo', 'run', '-q', '-p', 'raven-test-studio', '--'], via: 'o código deste repositório' }
  }
  const home = (await $.env.get('HOME')) ?? (await $.env.get('USERPROFILE')) ?? ''
  const programs = (await $.env.get('LOCALAPPDATA')) ?? ''
  const installed = [
    '/Applications/Raven Test Studio.app/Contents/MacOS/raven-test-studio',
    `${home}/Applications/Raven Test Studio.app/Contents/MacOS/raven-test-studio`,
    `${home}/.local/bin/raven-test-studio`,
    programs && `${programs}\\Programs\\RavenTestStudio\\raven-test-studio.exe`,
  ]
  for (const path of installed) {
    if (path && (await $.fs.exists(path))) return { argv: [path], via: 'o Studio instalado' }
  }
  return { argv: ['raven-test-studio'], via: 'o Studio do PATH' }
}

/** Runs a scenario with the Studio, with no window, and keeps its report. */
async function run($: Ops, ask: Ask): Promise<Last> {
  const found = await studio($)
  const argv = [...found.argv, '--run', ask.scenario, '--format', 'json']
  if (ask.noCache) argv.push('--no-cache')
  if (ask.delegate) argv.push('--delegate', ask.delegate)
  if (ask.delegate && ask.model) argv.push('--model', ask.model)
  await update($, running, () => true)
  await update($, last, now => ({ ...now, asked: ask.scenario, via: found.via, delegated: ask.delegate }))
  let outcome: Last
  const base = { asked: ask.scenario, via: found.via, delegated: ask.delegate }
  try {
    const done = await $.process.run(argv, { timeoutMs: 600_000 })
    if (done.exitCode === 0 || done.exitCode === 1) {
      outcome = { ...base, report: JSON.parse(done.stdout) as Report, problem: null }
    } else {
      outcome = { ...base, report: null, problem: done.stderr.trim().split('\n').slice(-6).join('\n') }
    }
  } catch (err) {
    outcome = { ...base, report: null, problem: String(err) }
  }
  await update($, last, () => outcome)
  await update($, running, () => false)
  // Running a scenario is when someone looks at the pane, so the card says what is true now:
  // a Studio updated by its window since the last check stops being offered the same version.
  void checkUpdate($)
  return outcome
}

/** Asks the Studio whether there is a newer version, and keeps the answer for the pane. */
async function checkUpdate($: Ops): Promise<void> {
  const found = await studio($)
  try {
    // Every Studio answers `--versao`; one before 0.1.2 would take `--atualizacao` for no option
    // and open its window, so it is asked the old way.
    const version = await $.process.run([...found.argv, '--versao'], { timeoutMs: 600_000 })
    const installed = version.stdout.trim().split('\n')[0]?.trim() ?? ''
    if (version.exitCode !== 0 || !installed) return
    if (isNewer(ASKS_FROM, installed)) {
      const answer = await olderStudio($, installed)
      if (answer) await update($, updates, now => ({ ...now, answer }))
      return
    }
    const done = await $.process.run([...found.argv, '--atualizacao', '--format', 'json'], { timeoutMs: 600_000 })
    if (done.exitCode === 0 || done.exitCode === 2) {
      const answer = JSON.parse(done.stdout) as Update['answer']
      await update($, updates, now => ({ ...now, answer }))
    }
  } catch {
    // A Studio that could not start: the pane says nothing about updates.
  }
}

/**
 * Asks again only when the Studio's version is not the one the card was written for, so the card
 * of a Studio updated by its window goes away without waiting for the hourly check.
 */
async function checkIfChanged($: Ops): Promise<void> {
  const known = (await read($, updates)).answer?.installed
  if (!known || (await read($, updates)).installing) return
  try {
    const found = await studio($)
    // In the checkout `--versao` would rebuild the Studio every few minutes; runs check there.
    if (found.argv[0] === 'cargo') return
    const version = await $.process.run([...found.argv, '--versao'], { timeoutMs: 600_000 })
    const installed = version.stdout.trim().split('\n')[0]?.trim() ?? ''
    if (version.exitCode === 0 && installed && installed !== known) await checkUpdate($)
  } catch {
    // A Studio that could not start: the card stays as it was.
  }
}

// The first Studio that answers `--atualizacao` and installs itself from the pane.
const ASKS_FROM = '0.1.2'
// Where the repository announces the latest version, the address the Studio's updater reads.
const NEWS_URL = 'https://raw.githubusercontent.com/alexkads/RavenRS/master/tools/raven-test-studio/novidades.json'

/** Whether `a` is a newer version than `b`, number by number. */
function isNewer(a: string, b: string): boolean {
  const x = a.split('.').map(Number)
  const y = b.split('.').map(Number)
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0)
  }
  return false
}

/**
 * A Studio from before 0.1.2 has no `--atualizacao`: the pane asks it `--versao`, reads the
 * news itself with `curl`, and says so; such a copy installs from its window, not from here.
 */
async function olderStudio($: Ops, installed: string): Promise<Update['answer']> {
  const fetched = await $.process.run(['curl', '-fsSL', NEWS_URL], { timeoutMs: 60_000 })
  if (fetched.exitCode !== 0) return null
  const news = JSON.parse(fetched.stdout) as { versao: string; titulo: string; novidades: string[]; por_que_atualizar: string }
  const newer = isNewer(news.versao, installed)
  return {
    installed,
    latest: news.versao,
    newer,
    updates_itself: false,
    old: true,
    news: newer
      ? { title: news.titulo, items: news.novidades, why: news.por_que_atualizar, important: false, issues: [] }
      : null,
    error: null,
  }
}

/**
 * Installs the newer version with `--atualizar`, which runs the Studio's own installer: as long
 * as compiling takes, so it streams, with no time limit, showing each stage it prints.
 */
async function installUpdate($: Ops): Promise<void> {
  const found = await studio($)
  await update($, updates, now => ({ ...now, installing: true, stage: 'baixando o instalador', done: null, failed: null }))
  const said: string[] = []
  let errors = ''
  const stream = $.process.spawn({ argv: [...found.argv, '--atualizar'] })
  let step = await stream.next()
  while (!step.done) {
    const { stream: which, text } = step.value
    if (which === 'stderr') errors += text
    for (const line of text.split('\n')) {
      said.push(line)
      const stage = line.startsWith('etapa: ') ? line.slice('etapa: '.length).trim() : null
      if (stage) await update($, updates, now => ({ ...now, stage }))
    }
    step = await stream.next()
  }
  const ok = step.value.code === 0
  const finished = said.find(line => line.startsWith('Atualizado para')) ?? 'Já está na versão mais nova'
  await update($, updates, now => ({
    ...now,
    installing: false,
    stage: null,
    done: ok ? finished : null,
    failed: ok ? null : errors.trim().split('\n').slice(-8).join('\n') || 'a atualização não terminou',
  }))
  if (ok) await checkUpdate($)
}

/** The name of an agent the Studio delegates to. */
function agentName(agent: string): string {
  return agent === 'claude' ? 'Claude' : 'Codex'
}

/** The report in a few lines the model reads, with the whole JSON after them. */
function forTheModel(outcome: Last): string {
  const r = outcome.report
  if (!r) return `Raven Test Studio could not run "${outcome.asked}": ${outcome.problem}`
  const lines = [`${r.scenario}: ${r.verdict === 'passed' ? 'passou' : 'falhou'} (${r.summary})${r.from_cache ? ', do cache' : ''}`]
  const d = r.delegation
  if (d) {
    lines.push(
      `delegado ao ${agentName(d.agent)} (${d.model ?? 'modelo padrão'}), que disse ${d.verdict ?? 'nada no formato pedido'}; ` +
        (d.confirmed ? 'o Studio rodou o mesmo cenário e confirmou' : 'o Studio rodou o mesmo cenário e NÃO confirmou'),
    )
    if (d.said?.cause) lines.push(`causa segundo o ${agentName(d.agent)}: ${d.said.cause}`)
    if (d.said?.fix) lines.push(`correção segundo o ${agentName(d.agent)}: ${d.said.fix}`)
  }
  const failed = r.blocks.find(b => b.result === 'failed')
  if (failed) {
    lines.push(`falhou no bloco ${failed.step} "${failed.title}" (${failed.kind}: ${failed.line}): ${failed.error}`)
    if (failed.fix) lines.push(`como corrigir: ${failed.fix}`)
  }
  lines.push(`não verificado: ${r.not_verified.join('; ')}`)
  lines.push('', JSON.stringify(r))
  return lines.join('\n')
}

/** What to ask the AI when a block failed. */
function askToFix(r: Report, b: Block): string {
  return [
    `O Raven Test Studio falhou no cenário "${r.scenario}", bloco ${b.step} "${b.title}" (${b.kind}: ${b.line}).`,
    `${b.error}`,
    b.fix ? `Dica: ${b.fix}` : '',
    b.screen ? `Tela do app testado:\n${b.screen}` : '',
    'Descubra a causa no código do RavenRS ou no cenário, corrija e rode o cenário de novo com a ferramenta run_scenario.',
  ]
    .filter(Boolean)
    .join('\n')
}

/** The blocks grouped by the Open block that starts each part of the scenario. */
function phases(blocks: Block[]): { title: string; blocks: Block[] }[] {
  const out: { title: string; blocks: Block[] }[] = []
  for (const b of blocks) {
    if (b.kind === 'open' || out.length === 0) out.push({ title: b.title, blocks: [] })
    out.at(-1)?.blocks.push(b)
  }
  return out
}

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    await $.command.register({ name: 'raven-test-studio', description: 'Abre o Raven Test Studio ao lado da conversa' })
    await $.tool.register({
      name: 'run_scenario',
      description:
        'Runs a Raven Test Studio (RTS) scenario with no window and returns its report: the verdict, each block with its result, the stable error code (E2E_*), what it means, how to fix it and the screen of the app under test when a block failed. "autoteste" is the Studio testing another Studio; "financas" runs the .raventest scenarios Raven Finanças brings in its own tests/e2e, on the real app opened apart in a child process with no window on its sample data in memory. Any other scenario is a .raventest file or a folder of them, in an app or in a folder of the QA team: pass its path. A Studio installed from scratch brings no test scenarios. When the user says RTS they mean this Studio. A scenario that did not change on a build that did not change comes back from the Studio cache unless no_cache is true. With delegate "codex" (or "claude") the Studio hands the scenario to that agent with its cheapest model, reading only: it runs the scenario, reads the report and finds the cause of a failure in the Rust sources; then the Studio runs it itself and says whether it confirms the agent. Delegate when the user asks for it or to save your own context.',
      inputSchema: {
        type: 'object',
        properties: {
          scenario: {
            type: 'string',
            description:
              'autoteste, financas, or the absolute path of a .raventest file or of a folder of them (an app\'s tests/e2e or a QA team\'s folder)',
          },
          no_cache: { type: 'boolean', description: 'Run again even when the cache has the answer' },
          delegate: {
            type: 'string',
            enum: ['codex', 'claude'],
            description: 'Hand the scenario to this coding agent, with its cheapest model',
          },
          model: { type: 'string', description: 'Another model for the agent delegated to; its cheapest by default' },
        },
        required: ['scenario'],
      },
      isDeferred: false,
    })
    await $.tool.register({
      name: 'scan_project',
      description:
        'Points Raven Test Studio (RTS) at a Rust project folder and says what can be tested there, without building it: the packages made with RavenRS, which are apps, whether each opens on the test channel, its .raventest scenarios and the views it builds with file, line and test ids. With action "run_all" it runs the scenarios of every app; with "suggest", an AI (local by default; codex or claude with delegate, cheapest model, reading only) reads the app\'s first screen and sources and writes new scenarios, which the Studio runs to say which pass; write keeps them in tests/e2e/sugestoes/ of the app for a person to review. Use it when the user points the RTS at a project, or when an app has no scenario yet.',
      inputSchema: {
        type: 'object',
        properties: {
          folder: { type: 'string', description: 'The absolute path of the project folder' },
          action: { type: 'string', enum: ['list', 'run_all', 'suggest'], description: 'What to do; list by default' },
          delegate: { type: 'string', enum: ['codex', 'claude'], description: 'For suggest: who writes instead of the local model' },
          write: { type: 'boolean', description: 'For suggest: keep the scenarios in tests/e2e/sugestoes/' },
        },
        required: ['folder'],
      },
      isDeferred: false,
    })
    void $.ui.open({ id: PANE, title: TITLE })
    // The Studio tests itself as the session starts, so the pane opens with its health; an
    // unchanged Studio answers from its cache at once.
    // The run asks about updates as it ends.
    void run($, { scenario: 'autoteste', noCache: false, delegate: null, model: null })
    // And looks for a newer Studio every hour after, as its window does.
    $.clock.every(CHECK_EVERY_MS, () => void checkUpdate($))
    // In between, a Studio updated elsewhere is seen within minutes.
    $.clock.every(VERSION_EVERY_MS, () => void checkIfChanged($))
    return next(e)
  })

  on('command.run', { command: 'raven-test-studio' }, async $ => {
    await $.ui.open({ id: PANE, title: TITLE, focus: true })
    return { text: 'Raven Test Studio aberto ao lado da conversa.' }
  })

  on('tool.call', { tool: TOOL }, async ($, e) => {
    const input = e as unknown as { scenario?: string; no_cache?: boolean; delegate?: string; model?: string }
    const asked = (input.scenario ?? '').trim()
    // A name the Studio knows, or a path of scenario files; the Studio is run with no shell.
    const scenario =
      SCENARIOS.includes(asked as (typeof SCENARIOS)[number]) || (asked.startsWith('/') && !asked.includes('\n'))
        ? asked
        : 'autoteste'
    const delegate = input.delegate === 'codex' || input.delegate === 'claude' ? input.delegate : null
    const model = typeof input.model === 'string' && /^[\w.:-]+$/.test(input.model) ? input.model : null
    void $.ui.open({ id: PANE, title: TITLE })
    const outcome = await run($, { scenario, noCache: input.no_cache === true, delegate, model })
    const newer = (await read($, updates)).answer
    const note =
      newer?.newer && newer.latest
        ? `\n\nHá uma versão nova do Raven Test Studio (${newer.installed} → ${newer.latest}); o painel oferece "Atualizar agora".`
        : ''
    return { result: forTheModel(outcome) + note }
  })

  on('tool.call', { tool: PROJECT_TOOL }, async ($, e) => {
    const input = e as unknown as { folder?: string; action?: string; delegate?: string; write?: boolean }
    const folder = (input.folder ?? '').trim()
    if (!folder.startsWith('/') || folder.includes('\n')) return { result: 'Diga o caminho absoluto da pasta do projeto.' }
    const found = await studio($)
    const argv = [...found.argv, '--project', folder, '--format', 'json']
    if (input.action === 'run_all') argv.push('--run-all')
    if (input.action === 'suggest') {
      argv.push('--suggest')
      if (input.delegate === 'codex' || input.delegate === 'claude') argv.push('--delegate', input.delegate)
      if (input.write === true) argv.push('--write')
    }
    const done = await $.process.run(argv, { timeoutMs: 600_000 })
    const said = done.stdout.trim() || done.stderr.trim()
    return { result: `Raven Test Studio (${found.via}), --project ${folder}: saída ${done.exitCode}\n${said}` }
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const { Box, Text, Button } = $.ui.resolve(e)
    const now = await read($, last)
    const busy = await read($, running)
    const r = now.report
    const press = (scenario: string, noCache: boolean, delegate: string | null = null) => () =>
      void run($, { scenario, noCache, delegate, model: null })
    const d = r?.delegation ?? null
    const failed = r?.blocks.find(b => b.result === 'failed')
    const ok = r?.verdict === 'passed'
    const passedCount = r ? r.blocks.filter(b => b.result === 'passed').length : 0
    const failedCount = r ? r.blocks.filter(b => b.result === 'failed').length : 0
    const skipped = r ? r.total - r.blocks.length : 0
    const asked = now.asked ?? 'autoteste'

    // The design's pieces (page "Painel no Claude" of the canvas, P1–P4), in the theme's colors:
    // a filled card, an outlined card, a section caption and a pill for the verdict.
    const card = (key: string, children: RenderChildren, border: string = LINE) => (
      <Box key={key} flexDirection="column" gap={1} backgroundColor={CARD} borderStyle="round" borderColor={border} paddingX={1} paddingY={1}>
        {children}
      </Box>
    )
    const outlined = (key: string, children: RenderChildren) => (
      <Box key={key} flexDirection="column" gap={0} borderStyle="round" borderColor={LINE} paddingX={1}>
        {children}
      </Box>
    )
    const caption = (text: string) => (
      <Text size="caption" weight="semibold" dimColor>
        {text.toUpperCase()}
      </Text>
    )
    const pill = (text: string, color: string) => (
      <Box backgroundColor={color} paddingX={1}>
        <Text size="caption" bold color="inverseText">
          {text}
        </Text>
      </Box>
    )

    // The header: the mark in the brand's color, the name, what is shown, and the verdict.
    const header = (
      <Box flexDirection="row" alignItems="flex-start" justifyContent="space-between" gap={1}>
        <Box flexDirection="column">
          <Text size="heading" weight="semibold">
            <Text color={BRAND}>◆</Text> Raven Test Studio
          </Text>
          <Text size="caption" dimColor>
            {busy ? titleOf(asked) : r ? r.scenario : 'Testes ponta a ponta do RavenRS, pelo próprio RavenRS'}
          </Text>
        </Box>
        {busy && pill('● RODANDO', 'warning')}
        {!busy && r && pill(ok ? '✓ PASSOU' : '✗ FALHOU', ok ? 'success' : 'error')}
      </Box>
    )

    // A newer Studio, said on every view of the pane, under the header.
    const up = await read($, updates)
    const a = up.answer
    const updateCard =
      up.installing || up.done || up.failed || (a?.newer && a.latest)
        ? card(
            'update',
            <Box flexDirection="column" gap={1}>
              {up.installing ? (
                <Box flexDirection="column" gap={0}>
                  <Text weight="semibold" color="warning">
                    ● Atualizando para {a?.latest ?? 'a versão nova'}
                  </Text>
                  <Text size="footnote" dimColor>
                    {up.stage ?? 'começando'} · compilar demora alguns minutos
                  </Text>
                </Box>
              ) : up.done ? (
                <Text weight="semibold" color="success">
                  ✓ {up.done}
                </Text>
              ) : up.failed ? (
                <Box flexDirection="column" gap={0}>
                  <Text weight="semibold" color="error">
                    A atualização falhou; o Studio instalado continua como estava
                  </Text>
                  <Text size="caption" mono dimColor>
                    {up.failed}
                  </Text>
                </Box>
              ) : (
                <Box flexDirection="column" gap={0}>
                  <Text weight="semibold">
                    Versão {a?.latest} disponível <Text dimColor>· você tem a {a?.installed}</Text>
                  </Text>
                  {a?.news?.title && <Text size="footnote">{a.news.title}</Text>}
                  {(a?.news?.items ?? []).slice(0, 3).map(item => (
                    <Text key={`new-${item}`} size="footnote" dimColor>
                      · {item}
                    </Text>
                  ))}
                </Box>
              )}
              {!up.installing && !up.done && a?.newer && (
                a.updates_itself ? (
                  <Button key="update-now" onPress={() => void installUpdate($)}>
                    Atualizar agora
                  </Button>
                ) : a.old ? (
                  <Text size="footnote" dimColor>
                    Esta versão do Studio atualiza pela janela: abra o Raven Test Studio e use "Atualizar" na barra de
                    status. Da 0.1.2 em diante, atualiza daqui.
                  </Text>
                ) : (
                  <Text size="footnote" dimColor>
                    Esta cópia é do checkout do RavenRS: atualize com <Text mono>git pull</Text>
                  </Text>
                )
              )}
            </Box>,
            up.failed ? 'error' : up.done ? 'success' : 'warning',
          )
        : null

    const footer = (
      <Text size="caption" mono dimColor>
        Feito com RavenRS · ravenrs-e2e
        {busy || !r
          ? busy
            ? ''
            : ' · /raven-test-studio reabre este painel'
          : ` · ${r.ran} blocos em ${r.duration_ms} ms${r.from_cache ? ' (do cache)' : ''}`}
      </Text>
    )
    // What is left of the pane's height, so the footer sits at the bottom as in the design.
    const spacer = <Box flexGrow={1} />
    const page = (children: RenderChildren) => (
      <Box flexDirection="column" gap={1} paddingX={1} paddingY={1} minHeight="100%">
        {children}
      </Box>
    )

    // P2: running, with the last run dimmed below.
    if (busy) {
      return page(
        <Box flexDirection="column" gap={1}>
          {header}
          {updateCard}
          {card(
            'steps',
            <Box flexDirection="column" gap={1}>
              <Text weight="semibold">{now.delegated ? `Delegado ao ${agentName(now.delegated)}` : 'Rodando o cenário'}</Text>
              <Text>
                <Text color="success">✓</Text> Usando {now.via ?? 'o Studio'}
              </Text>
              {now.delegated ? (
                <Text>
                  <Text color="warning">●</Text> Com o modelo mais barato <Text dimColor>· ele roda, lê o relatório e procura a causa</Text>
                </Text>
              ) : (
                <Text>
                  <Text color="warning">●</Text> Rodando, sem janela
                  <Text dimColor>{now.via === 'o código deste repositório' ? ' · compilar a primeira vez demora mais' : ''}</Text>
                </Text>
              )}
              <Text dimColor>○ Relatório para o painel e para a IA</Text>
            </Box>,
          )}
          {r &&
            outlined(
              'last',
              <Box flexDirection="column" gap={1}>
                {caption('Última execução')}
                <Box flexDirection="row" justifyContent="space-between" gap={1}>
                  <Text size="footnote" dimColor>
                    {r.scenario}
                  </Text>
                  <Text size="footnote" tabular color={ok ? 'success' : 'error'}>
                    {ok ? '✓' : '✗'} {passedCount}/{r.total} · {r.duration_ms} ms
                  </Text>
                </Box>
              </Box>,
            )}
          {spacer}
          {footer}
        </Box>,
      )
    }

    // P1: nothing run yet.
    if (!r) {
      return page(
        <Box flexDirection="column" gap={1}>
          {header}
          {updateCard}
          {now.problem &&
            card(
              'problem',
              <Box flexDirection="column" gap={1}>
                <Text weight="semibold" color="error">
                  Não deu para rodar "{titleOf(now.asked ?? 'autoteste')}"
                </Text>
                <Text size="footnote" dimColor>
                  {now.problem}
                </Text>
              </Box>,
              'error',
            )}
          {card(
            'self',
            <Box flexDirection="column" gap={1}>
              <Text size="heading" weight="semibold">
                {ABOUT.autoteste.title}
              </Text>
              <Text size="footnote" dimColor>
                {ABOUT.autoteste.blurb}
              </Text>
              <Button key="self" variant="primary" onPress={press('autoteste', false)}>
                ▶ Rodar autoteste
              </Button>
              <Button key="self-codex" dimColor onPress={press('autoteste', true, 'codex')}>
                Delegar ao Codex (modelo mais barato)
              </Button>
            </Box>,
          )}
          <Box flexDirection="column" gap={1}>
            {caption('Outros cenários')}
            {(['financas'] as const).map(key => (
              <Box key={`row-${key}`} flexDirection="row" alignItems="flex-start" gap={1} borderStyle="round" borderColor={LINE} paddingX={1}>
                <Text>{ABOUT[key].icon}</Text>
                <Box flexDirection="column" flexGrow={1} flexShrink={1}>
                  <Button key={`run-${key}`} plain onPress={press(key, false)}>
                    <Text weight="semibold">{ABOUT[key].title}</Text>
                  </Button>
                  <Text size="footnote" dimColor>
                    {ABOUT[key].blurb}
                  </Text>
                </Box>
                <Text dimColor>›</Text>
              </Box>
            ))}
          </Box>
          {outlined(
            'ai',
            <Box flexDirection="column" gap={1}>
              {caption('Para a IA')}
              <Text size="footnote">
                Peça na conversa: <Text italic>"roda o autoteste do Studio"</Text>. A IA chama a ferramenta{' '}
                <Text mono>run_scenario</Text>, recebe o relatório inteiro e o painel mostra o mesmo resultado.
              </Text>
            </Box>,
          )}
          {spacer}
          {footer}
        </Box>,
      )
    }

    // P3 and P4: a finished run.
    const metric = (key: string, label: string, value: string, color?: 'success' | 'error' | 'warning') => (
      <Box key={key} flexDirection="column" backgroundColor={CARD} borderStyle="round" borderColor={LINE} paddingX={1} flexGrow={1}>
        {caption(label)}
        <Text size="heading" weight="semibold" tabular color={color} dimColor={!color}>
          {value}
        </Text>
      </Box>
    )

    return page(
      <Box flexDirection="column" gap={1}>
        {header}
        {updateCard}

        {card(
          'score',
          <Box flexDirection="column" gap={1}>
            <Box flexDirection="row" justifyContent="space-between" alignItems="flex-end" gap={1}>
              <Text size="title" weight="semibold" tabular>
                {passedCount}
                <Text size="body" dimColor>
                  /{r.total}
                </Text>
              </Text>
              <Text size="footnote" dimColor>
                {ok ? 'passos passaram' : `falhou no passo ${failed?.step ?? '?'}`} · {r.duration_ms} ms
              </Text>
            </Box>
            <Text mono>
              {r.blocks.map(b => (
                <Text key={`bar-${b.id}`} color={b.result === 'passed' ? 'success' : 'error'}>
                  ■
                </Text>
              ))}
              {skipped > 0 && <Text color="subtle">{'■'.repeat(skipped)}</Text>}
            </Text>
            <Text size="caption" dimColor>
              {r.from_cache ? 'do cache: nem o cenário nem o build mudaram' : 'Um ■ por bloco do cenário, na ordem em que rodou'}
            </Text>
          </Box>,
        )}

        {d &&
          card(
            'delegation',
            <Box flexDirection="column" gap={1}>
              <Box flexDirection="row" justifyContent="space-between" gap={1}>
                <Text weight="semibold">
                  Delegado ao {agentName(d.agent)} <Text dimColor>· {d.model ?? 'modelo padrão'}</Text>
                </Text>
                <Text size="footnote" tabular dimColor>
                  {(d.duration_ms / 1000).toFixed(1)} s
                </Text>
              </Box>
              <Text size="footnote">
                Disse{' '}
                <Text weight="semibold">
                  {d.verdict === 'passed' ? 'passou' : d.verdict === 'failed' ? 'falhou' : 'nada no formato pedido'}
                </Text>
                {d.said?.summary ? ` — ${d.said.summary}` : ''}
              </Text>
              {d.said?.cause && (
                <Text size="footnote">
                  <Text weight="semibold">Causa:</Text> {d.said.cause}
                </Text>
              )}
              {d.said?.fix && (
                <Text size="footnote">
                  <Text weight="semibold">Como corrigir:</Text> {d.said.fix}
                </Text>
              )}
              <Text size="footnote" color={d.confirmed ? 'success' : 'warning'}>
                {d.confirmed
                  ? '✓ O Studio rodou o mesmo cenário e confirmou o veredito'
                  : '! O Studio rodou o mesmo cenário e chegou a outro veredito'}
              </Text>
            </Box>,
            d.confirmed ? 'success' : 'warning',
          )}

        {failed &&
          card(
            'failure',
            <Box flexDirection="column" gap={1}>
              <Box flexDirection="column">
                <Text weight="semibold" color="error">
                  ✗ Bloco {failed.step} · {failed.title}
                </Text>
                <Text size="footnote" dimColor>
                  {failed.kind} · {failed.line}
                </Text>
              </Box>
              <Box flexDirection="row" flexWrap="wrap" gap={1} alignItems="center">
                {failed.code && (
                  <Box borderStyle="round" borderColor="error" paddingX={1}>
                    <Text size="caption" weight="semibold" mono color="error">
                      {failed.code}
                    </Text>
                  </Box>
                )}
                <Text size="footnote">{failed.meaning ?? failed.error}</Text>
              </Box>
              {failed.detail && (
                <Text size="footnote" dimColor>
                  {failed.detail}
                </Text>
              )}
              {failed.fix && (
                <Text size="footnote">
                  <Text weight="semibold">Como corrigir:</Text> {failed.fix}
                </Text>
              )}
              {failed.close.length > 0 && (
                <Box flexDirection="column">
                  {caption('Parecidos na tela')}
                  {failed.close.map(line => (
                    <Text key={`close-${line}`} size="footnote" mono dimColor>
                      {line}
                    </Text>
                  ))}
                </Box>
              )}
              {failed.screen && (
                <Box flexDirection="column" gap={0}>
                  {caption('Tela do app testado')}
                  <Box backgroundColor={DEEP} paddingX={1} paddingY={1}>
                    <Text size="caption" mono dimColor>
                      {failed.screen.trimEnd()}
                    </Text>
                  </Box>
                </Box>
              )}
              <Box flexDirection="row" gap={1}>
                <Button key="ask-ai" onPress={() => void $.prompt.submit({ text: askToFix(r, failed) })}>
                  Pedir à IA para corrigir
                </Button>
                <Button key="copy" dimColor onPress={() => void $.ui.copy({ text: askToFix(r, failed), surface: e.surface })}>
                  Copiar
                </Button>
              </Box>
            </Box>,
            'error',
          )}

        <Box flexDirection="row" gap={1}>
          {metric('passed', 'Passaram', String(passedCount), 'success')}
          {metric('failed', 'Falharam', String(failedCount), failedCount > 0 ? 'error' : undefined)}
          {metric('skipped', 'Puladas', String(skipped), skipped > 0 ? 'warning' : undefined)}
          {metric('cache', 'Cache', r.from_cache ? 'sim' : 'não')}
        </Box>

        <Box flexDirection="column" gap={1}>
          <Button key="again" variant="primary" onPress={press(asked, false)}>
            {ok ? '▶ Rodar autoteste' : '▶ Rodar de novo'}
          </Button>
          <Box flexDirection="row" flexWrap="wrap" gap={1}>
            <Button key="fresh" onPress={press(asked, true)}>
              ↻ Sem cache
            </Button>
            {asked !== 'autoteste' && (
              <Button key="self" onPress={press('autoteste', false)}>
                Autoteste
              </Button>
            )}
            {asked !== 'financas' && (
              <Button key="finance" onPress={press('financas', false)}>
                Raven Finanças
              </Button>
            )}

            <Button key="codex" dimColor onPress={press(asked, true, 'codex')}>
              Delegar ao Codex
            </Button>
          </Box>
        </Box>

        <Box flexDirection="column" gap={1}>
          {phases(r.blocks).map((phase, i) => {
            const failing = phase.blocks.some(b => b.result === 'failed')
            const shown = failing ? phase.blocks : phase.blocks.slice(0, SHOWN)
            const rest = phase.blocks.length - shown.length
            return (
              <Box key={`phase-${i}`} flexDirection="column">
                {caption(`${phase.title} · ${phase.blocks.length} bloco${phase.blocks.length === 1 ? '' : 's'}`)}
                {shown.map(b => (
                  <Text key={`row-${b.id}`} size="footnote" color={b.result === 'failed' ? 'error' : undefined}>
                    <Text color={b.result === 'passed' ? 'success' : 'error'}>{b.result === 'passed' ? '✓' : '✗'}</Text>{' '}
                    <Text tabular dimColor>
                      {String(b.step).padStart(2, ' ')}
                    </Text>{' '}
                    {b.title}
                  </Text>
                ))}
                {rest > 0 && (
                  <Text size="footnote" dimColor>
                    <Text color="success">✓</Text> … mais {rest}
                  </Text>
                )}
              </Box>
            )
          })}
          {skipped > 0 && (
            <Text size="footnote" color="warning">
              ○ {skipped} bloco{skipped === 1 ? '' : 's'} não rodou depois da falha
            </Text>
          )}
        </Box>

        {outlined(
          'not-verified',
          <Box flexDirection="column" gap={1}>
            {caption('Não verificado')}
            {r.not_verified.map(line => (
              <Text key={`nv-${line}`} size="caption" dimColor>
                · {line}
              </Text>
            ))}
          </Box>,
        )}

        {spacer}
        {footer}
      </Box>,
    )
  })
}
