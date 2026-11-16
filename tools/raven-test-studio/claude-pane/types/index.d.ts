/** One block of a run, as the Studio's JSON report gives it. */
export type Block = {
  step: number
  id: string
  kind: string
  title: string
  line: string
  result: 'passed' | 'failed'
  code: string | null
  error: string | null
  meaning: string | null
  detail: string | null
  fix: string | null
  close: string[]
  screen: string | null
}

/** A run of a scenario, as `raven-test-studio --run <name> --format json` prints it. */
export type Report = {
  scenario: string
  verdict: 'passed' | 'failed'
  summary: string
  total: number
  ran: number
  blocks: Block[]
  duration_ms: number
  from_cache: boolean
  cached_at: number | null
  not_verified: string[]
  /** Present when the scenario was handed to another agent (`--delegate`). */
  delegation?: Delegation
}

/** What the agent a scenario was handed to said, and whether the Studio confirmed it. */
export type Delegation = {
  agent: string
  model: string | null
  verdict: 'passed' | 'failed' | null
  confirmed: boolean
  said: { summary?: string; cause?: string | null; fix?: string | null; code?: string | null } | null
  message: string
  commands: string[]
  session: string | null
  duration_ms: number
}

/** What the pane shows: the last report, or why there is none, and whether a run is going. */
export type Last = {
  report: Report | null
  problem: string | null
  asked: string | null
  /** Which Studio ran it: the code of this checkout, the installed one, or the one in PATH. */
  via: string | null
  /** The agent the run was handed to, if any. */
  delegated: string | null
}

declare module 'claude-code' {
  interface PluginState {
    'raven-test-studio': {
      last: Shaped<Last>
      running: Shaped<boolean>
    }
  }
}
