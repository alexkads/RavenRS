import { atom, read, update } from 'claude-code'
import type { Hook, Register } from 'claude-code'

import type { Block, Last, Report } from '../types'

const PANE = 'raven-test-studio'
const TITLE = 'Raven Test Studio'
const TOOL = 'mcp__raven-test-studio__run_scenario'
// The scenarios that come with the Studio, by the name its `--run` takes.
const SCENARIOS = ['autoteste', 'lancamento', 'quebrado'] as const
// What each scenario is, for the empty pane.
const ABOUT = {
  autoteste: {
    title: 'O Studio testa o Studio',
    blurb: 'Abre outro Studio, roda o cenário dele, confere cada bloco, para, anda passo a passo e confere uma falha de propósito. 18 blocos.',
  },
  lancamento: { title: 'Lançar despesa', blurb: 'preenche e salva um formulário de exemplo · 5 blocos' },
  quebrado: { title: 'Cenário quebrado', blurb: 'falha no passo 2 de propósito, para ver um erro · 3 blocos' },
} as const

/** The title of a scenario by the name `--run` takes. */
function titleOf(name: string): string {
  return name in ABOUT ? ABOUT[name as keyof typeof ABOUT].title : name
}
// Blocks a group shows before "mais N".
const SHOWN = 4

const last = atom(
  { plugin: 'raven-test-studio', key: 'last' } as const,
  { report: null, problem: null, asked: null, via: null, delegated: null } as Last,
  { shape: 'last-3' },
)
const running = atom({ plugin: 'raven-test-studio', key: 'running' } as const, false, { shape: 'running-1' })

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
    const done = await $.process.run(argv, { timeoutMs: 900_000 })
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
  return outcome
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
        'Runs a Raven Test Studio scenario with no window and returns its report: the verdict, each block with its result, the stable error code (E2E_*), what it means, how to fix it and the screen of the app under test when a block failed. "autoteste" is the Studio testing another Studio; "lancamento" fills in a sample form; "quebrado" fails on purpose. A scenario that did not change on a build that did not change comes back from the Studio cache unless no_cache is true. With delegate "codex" (or "claude") the Studio hands the scenario to that agent with its cheapest model, reading only: it runs the scenario, reads the report and finds the cause of a failure in the Rust sources; then the Studio runs it itself and says whether it confirms the agent. Delegate when the user asks for it or to save your own context.',
      inputSchema: {
        type: 'object',
        properties: {
          scenario: { type: 'string', enum: [...SCENARIOS], description: 'Which scenario to run' },
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
    void $.ui.open({ id: PANE, title: TITLE })
    // The Studio tests itself as the session starts, so the pane opens with its health; an
    // unchanged Studio answers from its cache at once.
    void run($, { scenario: 'autoteste', noCache: false, delegate: null, model: null })
    return next(e)
  })

  on('command.run', { command: 'raven-test-studio' }, async $ => {
    await $.ui.open({ id: PANE, title: TITLE, focus: true })
    return { text: 'Raven Test Studio aberto ao lado da conversa.' }
  })

  on('tool.call', { tool: TOOL }, async ($, e) => {
    const input = e as unknown as { scenario?: string; no_cache?: boolean; delegate?: string; model?: string }
    const scenario = SCENARIOS.includes((input.scenario ?? '') as (typeof SCENARIOS)[number]) ? input.scenario! : 'autoteste'
    const delegate = input.delegate === 'codex' || input.delegate === 'claude' ? input.delegate : null
    const model = typeof input.model === 'string' && /^[\w.:-]+$/.test(input.model) ? input.model : null
    void $.ui.open({ id: PANE, title: TITLE })
    const outcome = await run($, { scenario, noCache: input.no_cache === true, delegate, model })
    return { result: forTheModel(outcome) }
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

    // The header: the mark, the name, what is shown, and the verdict as a pill.
    const header = (
      <Box flexDirection="row" alignItems="flex-start" justifyContent="space-between" gap={1}>
        <Box flexDirection="column">
          <Text size="heading" bold>
            <Text color="claude">◆</Text> Raven Test Studio
          </Text>
          <Text size="caption" dimColor>
            {r ? r.scenario : 'Testes ponta a ponta do RavenRS, pelo próprio RavenRS'}
          </Text>
        </Box>
        {busy && (
          <Box backgroundColor="warning" paddingX={1}>
            <Text bold color="inverseText">
              ● RODANDO
            </Text>
          </Box>
        )}
        {!busy && r && (
          <Box backgroundColor={ok ? 'success' : 'error'} paddingX={1}>
            <Text bold color="inverseText">
              {ok ? '✓ PASSOU' : '✗ FALHOU'}
            </Text>
          </Box>
        )}
      </Box>
    )

    const footer = (
      <Text size="caption" dimColor>
        Feito com RavenRS · ravenrs-e2e
        {r ? ` · ${r.ran} blocos em ${r.duration_ms} ms${r.from_cache ? ' (do cache)' : ''}` : ' · /raven-test-studio reabre este painel'}
      </Text>
    )

    // P2: running, with the last run dimmed below.
    if (busy) {
      return (
        <Box flexDirection="column" gap={1} paddingX={1} paddingY={1}>
          {header}
          <Box flexDirection="column" borderStyle="round" borderColor="subtle" paddingX={1} paddingY={1}>
            <Text bold>{titleOf(asked)}</Text>
            <Text>
              <Text color="success">✓</Text> Usando {now.via ?? 'o Studio'}
            </Text>
            {now.delegated ? (
              <Text>
                <Text color="warning">●</Text> Delegado ao {agentName(now.delegated)} com o modelo mais barato{' '}
                <Text dimColor>· ele roda, lê o relatório e procura a causa</Text>
              </Text>
            ) : (
              <Text>
                <Text color="warning">●</Text> Rodando, sem janela{' '}
                <Text dimColor>{now.via === 'o código deste repositório' ? '· compilar a primeira vez demora mais' : ''}</Text>
              </Text>
            )}
            <Text dimColor>○ Relatório para o painel e para a IA</Text>
          </Box>
          {r && (
            <Box flexDirection="column" borderStyle="round" borderColor="subtle" paddingX={1}>
              <Text size="caption" dimColor>
                ÚLTIMA EXECUÇÃO
              </Text>
              <Text dimColor>
                {r.scenario} · {ok ? '✓' : '✗'} {passedCount}/{r.total} · {r.duration_ms} ms
              </Text>
            </Box>
          )}
          {footer}
        </Box>
      )
    }

    // P1: nothing run yet.
    if (!r) {
      return (
        <Box flexDirection="column" gap={1} paddingX={1} paddingY={1}>
          {header}
          {now.problem && (
            <Box flexDirection="column" borderStyle="round" borderColor="error" paddingX={1}>
              <Text bold color="error">
                Não deu para rodar "{now.asked}"
              </Text>
              <Text dimColor>{now.problem}</Text>
            </Box>
          )}
          <Box flexDirection="column" borderStyle="round" borderColor="subtle" paddingX={1} paddingY={1} gap={1}>
            <Text bold>{ABOUT.autoteste.title}</Text>
            <Text dimColor>{ABOUT.autoteste.blurb}</Text>
            <Button key="self" variant="primary" onPress={press('autoteste', false)}>
              ▶ Rodar autoteste
            </Button>
            <Button key="self-codex" onPress={press('autoteste', true, 'codex')}>
              Delegar ao Codex (modelo mais barato)
            </Button>
          </Box>
          <Box flexDirection="column" gap={1}>
            <Text size="caption" dimColor>
              OUTROS CENÁRIOS
            </Text>
            {(['lancamento', 'quebrado'] as const).map(key => (
              <Box key={key} flexDirection="row" justifyContent="space-between" alignItems="center" gap={1}>
                <Box flexDirection="column" flexShrink={1}>
                  <Text bold>{ABOUT[key].title}</Text>
                  <Text size="caption" dimColor>
                    {ABOUT[key].blurb}
                  </Text>
                </Box>
                <Button key={`run-${key}`} onPress={press(key, false)}>
                  Rodar
                </Button>
              </Box>
            ))}
          </Box>
          <Box flexDirection="column" borderStyle="round" borderColor="subtle" paddingX={1}>
            <Text size="caption" dimColor>
              PARA A IA
            </Text>
            <Text>
              Peça na conversa: "roda o autoteste do Studio". A IA chama a ferramenta run_scenario, recebe o relatório
              inteiro e o painel mostra o mesmo resultado.
            </Text>
          </Box>
          {footer}
        </Box>
      )
    }

    // P3 and P4: a finished run.
    const metric = (key: string, label: string, value: string, color?: 'success' | 'error' | 'warning') => (
      <Box key={key} flexDirection="column" borderStyle="round" borderColor="subtle" paddingX={1} flexGrow={1}>
        <Text size="caption" dimColor>
          {label}
        </Text>
        <Text size="heading" bold color={color} dimColor={!color}>
          {value}
        </Text>
      </Box>
    )

    return (
      <Box flexDirection="column" gap={1} paddingX={1} paddingY={1}>
        {header}

        <Box flexDirection="column" borderStyle="round" borderColor="subtle" paddingX={1}>
          <Box flexDirection="row" justifyContent="space-between" alignItems="flex-end" gap={1}>
            <Text size="title" bold>
              {passedCount}
              <Text dimColor>/{r.total}</Text>
            </Text>
            <Text dimColor>
              {ok ? 'passos passaram' : `falhou no passo ${failed?.step ?? '?'}`} · {r.duration_ms} ms
            </Text>
          </Box>
          <Text>
            {r.blocks.map(b => (
              <Text key={`bar-${b.id}`} color={b.result === 'passed' ? 'success' : 'error'}>
                ■
              </Text>
            ))}
            {skipped > 0 && <Text color="subtle">{'■'.repeat(skipped)}</Text>}
          </Text>
          {r.from_cache && <Text size="caption" dimColor>do cache: nem o cenário nem o build mudaram</Text>}
        </Box>

        {d && (
          <Box
            flexDirection="column"
            borderStyle="round"
            borderColor={d.confirmed ? 'success' : 'warning'}
            paddingX={1}
            gap={1}
          >
            <Box flexDirection="row" justifyContent="space-between" gap={1}>
              <Text bold>
                Delegado ao {agentName(d.agent)} <Text dimColor>· {d.model ?? 'modelo padrão'}</Text>
              </Text>
              <Text dimColor>{(d.duration_ms / 1000).toFixed(1)} s</Text>
            </Box>
            <Text>
              Disse <Text bold>{d.verdict === 'passed' ? 'passou' : d.verdict === 'failed' ? 'falhou' : 'nada no formato pedido'}</Text>
              {d.said?.summary ? ` — ${d.said.summary}` : ''}
            </Text>
            {d.said?.cause && (
              <Text>
                <Text bold>Causa:</Text> {d.said.cause}
              </Text>
            )}
            {d.said?.fix && (
              <Text>
                <Text bold>Como corrigir:</Text> {d.said.fix}
              </Text>
            )}
            <Text color={d.confirmed ? 'success' : 'warning'}>
              {d.confirmed
                ? '✓ O Studio rodou o mesmo cenário e confirmou o veredito'
                : '! O Studio rodou o mesmo cenário e chegou a outro veredito'}
            </Text>
          </Box>
        )}

        {failed && (
          <Box flexDirection="column" borderStyle="round" borderColor="error" paddingX={1} gap={1}>
            <Box flexDirection="column">
              <Text bold color="error">
                ✗ Bloco {failed.step} · {failed.title}
              </Text>
              <Text size="caption" dimColor>
                {failed.kind} · {failed.line}
              </Text>
            </Box>
            <Box flexDirection="row" flexWrap="wrap" gap={1} alignItems="center">
              {failed.code && (
                <Box borderStyle="round" borderColor="error" paddingX={1}>
                  <Text bold color="error">
                    {failed.code}
                  </Text>
                </Box>
              )}
              <Text>{failed.meaning ?? failed.error}</Text>
            </Box>
            {failed.detail && <Text dimColor>{failed.detail}</Text>}
            {failed.fix && (
              <Text>
                <Text bold>Como corrigir:</Text> {failed.fix}
              </Text>
            )}
            {failed.close.length > 0 && (
              <Box flexDirection="column">
                <Text size="caption" dimColor>
                  PARECIDOS NA TELA
                </Text>
                {failed.close.map(line => (
                  <Text key={`close-${line}`} dimColor>
                    {line}
                  </Text>
                ))}
              </Box>
            )}
            {failed.screen && (
              <Box flexDirection="column">
                <Text size="caption" dimColor>
                  TELA DO APP TESTADO
                </Text>
                <Text dimColor>{failed.screen.trimEnd()}</Text>
              </Box>
            )}
            <Box flexDirection="row" gap={1}>
              <Button key="ask-ai" onPress={() => void $.prompt.submit({ text: askToFix(r, failed) })}>
                Pedir à IA para corrigir
              </Button>
              <Button key="copy" onPress={() => void $.ui.copy({ text: askToFix(r, failed), surface: e.surface })}>
                Copiar
              </Button>
            </Box>
          </Box>
        )}

        <Box flexDirection="row" gap={1}>
          {metric('passed', 'PASSARAM', String(passedCount), 'success')}
          {metric('failed', 'FALHARAM', String(failedCount), failedCount > 0 ? 'error' : undefined)}
          {metric('skipped', 'PULADAS', String(skipped), skipped > 0 ? 'warning' : undefined)}
          {metric('cache', 'CACHE', r.from_cache ? 'sim' : 'não')}
        </Box>

        <Box flexDirection="column" gap={1}>
          <Button key="again" variant="primary" onPress={press(asked, false)}>
            {ok ? '▶ Rodar autoteste' : '▶ Rodar de novo'}
          </Button>
          <Box flexDirection="row" flexWrap="wrap" gap={1}>
            <Button key="fresh" onPress={press(asked, true)}>
              ↻ Sem cache
            </Button>
            <Button key="codex" onPress={press(asked, true, 'codex')}>
              Delegar ao Codex
            </Button>
            {asked !== 'autoteste' && (
              <Button key="self" onPress={press('autoteste', false)}>
                Autoteste
              </Button>
            )}
            {asked !== 'lancamento' && (
              <Button key="expense" onPress={press('lancamento', false)}>
                Lançar despesa
              </Button>
            )}
            {asked !== 'quebrado' && (
              <Button key="broken" onPress={press('quebrado', false)}>
                Quebrado
              </Button>
            )}
          </Box>
        </Box>

        <Box flexDirection="column" gap={1}>
          {phases(r.blocks).map((phase, i) => {
            const failing = phase.blocks.some(b => b.result === 'failed')
            const shown = failing ? phase.blocks : phase.blocks.slice(0, SHOWN)
            const rest = phase.blocks.length - shown.length
            return (
              <Box key={`phase-${i}`} flexDirection="column">
                <Text size="caption" dimColor>
                  {phase.title.toUpperCase()} · {phase.blocks.length} BLOCO{phase.blocks.length === 1 ? '' : 'S'}
                </Text>
                {shown.map(b => (
                  <Text key={`row-${b.id}`} color={b.result === 'failed' ? 'error' : undefined}>
                    <Text color={b.result === 'passed' ? 'success' : 'error'}>{b.result === 'passed' ? '✓' : '✗'}</Text>{' '}
                    <Text dimColor>{String(b.step).padStart(2, ' ')}</Text> {b.title}
                  </Text>
                ))}
                {rest > 0 && <Text dimColor>✓ … mais {rest}</Text>}
              </Box>
            )
          })}
          {skipped > 0 && <Text color="warning">○ {skipped} bloco(s) não rodaram depois da falha</Text>}
        </Box>

        <Box flexDirection="column" borderStyle="round" borderColor="subtle" paddingX={1}>
          <Text size="caption" dimColor>
            NÃO VERIFICADO
          </Text>
          {r.not_verified.map(line => (
            <Text key={`nv-${line}`} dimColor>
              · {line}
            </Text>
          ))}
        </Box>

        {footer}
      </Box>
    )
  })
}
