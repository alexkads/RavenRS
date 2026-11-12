# Contracts and acceptance scenarios for reusable modules

What a reusable component, module or plugin of `ravenrs-app` has to honor so that **any** application can install it, and the scenarios that prove it. The state of each capability lives in [APP-PARITY.md](APP-PARITY.md); this page says what “done” means. Coordination: [issue #253](https://github.com/alexkads/RavenRS/issues/253).

**Status: specification.** Nothing on this page is implemented by this page. Names in `code` that do not exist yet are illustrative; each implementation issue decides the final ones.

## Rules every package follows

- Public APIs, schemas, events and base examples use generic words: organization, unit, person, group, session, request, obligation, posting, document, resource.
- No generic package requires a concept of one sector. Sector presets live in optional packages, named as such.
- The application supplies eligibility, access, calendar, approval, billing and retention rules through documented contracts. A missing setting never grants wide access.
- A package never reads another application's private tables and never depends on its binary. It integrates through public APIs and the authorized context.
- At least two consumers from different sectors exercise the same package with different settings, without copying it. Changing one consumer never needs `if application == "..."` in the framework.

## Test fixture

Every scenario runs on synthetic data, with no private database, credential or account. [`ravenrs-app-testkit`](../crates/ravenrs-app-testkit) builds it: organizations, units, people, the example roles and a controlled clock.

| Record | Fields and relations |
|---|---|
| `Organization` | id, name. T1 and T2, isolated |
| `Unit` | id, organization, name. U1 and U2 in T1; U1 in T2 |
| `Person` | id, unit, name, contact, reserved note. A P1 in T1 and another P1 in T2 |
| `Membership` | person, group, start and end |
| `Group`, `Session` | unit, owners, eligible participants, start, end, time zone, state |
| `Request`, `Decision` | requester, kind, state, approver, deadline, reason, version |
| `Obligation`, `Posting` | amount in minor units, currency, state, source reference, uniqueness of the effect |
| `Document`, `Revision` | owner, audience, version, content hash, issue and signature state |
| `Resource`, `Reservation` | unit, availability, interval, preparation time, link to the request |

### Profiles of the example policy

The policy belongs to the test application, not to RavenRS. Composition and precedence of roles are documented and tested; nothing is inferred from a role's name or from a hidden menu.

| Profile | Granted | Refusal to prove |
|---|---|---|
| Administrator of T1 | administration of T1 | never enters T2, never becomes a global operator |
| Registrar of U1 | people and common data of U1 | no finance, reserved notes or people of U2 |
| Finance of U1 | obligations and postings of U1 | no reserved notes, no finance of U2 |
| Owner of G1 | sessions and attendance of G1 | cannot write to G2 by changing keys or references |
| Participant P1 | own profile, requests and what is shared | cannot reach the P1 of T2 or another person of T1 by id |
| Integration worker | only the operation and organization granted to the job | no universal administrative account |
| Revoked user | nothing new | sessions, files, subscriptions and MCP tools keep no earlier access |

### Conventions

- T1 and T2 use the **same local keys** on purpose, to catch mixing across connection, cache, file, job and event.
- Money is an integer in minor units: `10000` is 100 units when the currency scale is two. The application declares currency and scale; no contract fixes one currency.
- Times keep the instant and the time zone the rule needs. Tests use a controlled clock, never the machine's clock or language.
- Identity and organization come from a trusted source. An `organization_id`, author or role sent by the client grants nothing.
- A critical write is atomic within its documented boundary. External effects go through events and idempotency keys; a network failure does not mean the operation did not happen.

## Contract 1: module and host application

**Input:** module settings, an authorized data context, the access policy, language, clock and the providers the module needs.
**Registers:** entities and adapters, actions, screens, migrations, jobs, translations and capabilities.
**Output:** public surfaces the application can use, and clear configuration and compatibility errors.

`Module` (`crates/ravenrs-app/src/module.rs`) declares today its name, dependencies, entities, view and window controllers, model extensions, its model difference and initial records. It does not declare yet:

- version and compatible range, targets and databases supported;
- permissions, migrations, jobs, providers and capabilities it adds;
- conflicts and cycles between modules, found when the `Application` is built;
- turning a module off without deleting data and without granting access, keeping the reads and exports it promised.

A small application installs one module without the whole platform. A mobile client uses the service contract and never reads the server's SQLite directly.

## Contract 2: external provider

A provider declares the capabilities it supports, its settings and secrets, the authorized context it runs in, cancellation and timeout, its result and how to resume. A deterministic **fake provider** tests success, refusal, late answer, duplicate and outage with no paid account and no network.

In `ravenrs-app`, `Provider` names a provider and the capabilities it declares, and `EventSink` sends events under a key and, with `CAN_TELL_STATUS`, tells whether it already has one. Failures use the meanings of contract 3. `ravenrs_app_testkit::FakeSink` is the fake one.

Capabilities are honest: messaging may send without receiving, a printer may produce a document without confirming paper, storage may limit sizes, a player may not play every format on every host. The interface shows the effective capability and never shows a failure as success.

## Contract 3: commands, events and failures

Every operation that changes data documents its preconditions, permission, transaction boundary, idempotency, consequence, audit and the result shown to the person. An effect key belongs to its organization: a key of T1 never deduplicates an operation of T2.

The result distinguishes six meanings, all of them in `SpaceError` (`crates/ravenrs-app/src/space.rs`). `is_temporary()` says when trying again later may work, and `may_have_happened()` when doing it again could do it twice:

| Meaning | Today |
|---|---|
| Not allowed | `SpaceError::Denied` |
| Invalid data | `SpaceError::Refused`, `BadQuery`, `DuplicateKey` |
| Version conflict | `SpaceError::Conflict` |
| Capability unavailable | `SpaceError::Unavailable` |
| Temporary integration failure | `SpaceError::Temporary`; SQLite busy or locked by another connection answers it |
| Outcome still unknown | `SpaceError::Unknown` |

Messages never reveal that data the person cannot reach exists.

## Scenarios

Each scenario has one owner issue, where its checklist lives. The text of the interface may change; the observable result is the contract. Sketches V01–V16 and diagrams D01–D03 are in the [visual appendix of #253](https://github.com/alexkads/RavenRS/issues/253#issuecomment-6828194062).

| Scenario | What it proves | Owner |
|---|---|---|
| C01 | The same read through the local and the remote client returns the same allowed fields; a forged key of T2 is refused; paging and export keep the scope | #226 |
| C02 | A relation is authorized on create too, including references the storage computes; switching organization clears the previous context | #232, #225 |
| C03 | An external identity never widens scope: no linking by e-mail, revocation stops the next operation, provider outage has a documented recovery | #257 |
| C04 | A second writer on the same version does not overwrite silently; a backup of T1 restores on a clean install, a file of T2 is refused; migrations keep data and can be recovered | #241, #229, #243 |
| C05 | An offline change reconciles once; a permission revoked meanwhile is refused; an online-only command shows pending, never false success; turning a module off keeps reads and exports | #240, #242 |
| C06 | An approval survives a restart without losing or repeating a decision; self-approval follows the application's policy; delegation ends on its date | #258 |
| C07 | An event leaves only after its commit; a duplicate or a crash between send and confirmation never doubles the effect; an unauthenticated incoming event is refused | #254, #255, #256 |
| C08 | Conversations follow membership and the application's history policy; a repeated notification does not duplicate a message | #259 |
| C09 | A published form keeps its versions; an audience fires once per change; a field without read permission never leaks through filter, preview or count | #260 |
| C10 | An issued document stays frozen at its template revision, with its hash; download checks permission; a repeated signature callback changes nothing | #261 |
| C11 | Two requests for the last resource confirm one; one occurrence moves without the series; a staffing suggestion stays a proposal | #227 |
| C12 | A payment confirms the right obligation once; a partial refund keeps the original; a callback for the wrong organization is refused | #262 |
| C13 | A pass is validated by the service, not the scanner; reading it twice does not add attendance; printing and attendance have separate states | #263 |
| C14 | Map, list and report show the same allowed people; nothing of U2 appears anywhere; a failed geocoder keeps the address | #170 |
| C15 | A page has draft, version and audience; a direct address checks the audience; publishing installs without the other modules | #264 |
| C16 | Preview and public output stay apart; a disconnected screen recovers; the player states what each host can play | #237, #265 |
| C17 | Card, chart, list and export share filter and period; restricted is not zero; aggregation happens in storage; one failing tile does not break the rest | #203 |
| C18 | An assistant sees and changes only what its user can; a write shows its effect first and is checked again on confirmation | #266 |
| C19 | One package runs in two applications of different sectors, with different entities, texts, roles and policies, without copying it | #268 |

## First slice

[`ravenrs-app-requests`](../crates/ravenrs-app-requests) is the first package held to these contracts (#268). Its tests prove, on the fixture:

- **C02**, as far as `master` goes: a registrar of U1 reads only the people of U1, never the reserved note, and cannot create a person in U2; T1 and T2 never mix although they share keys; a worker reaches only the entities of its job. References computed on create wait for #232.
- **C06, minimum:** the role of each step and the application's own rule decide; a decision on an old version is a conflict; reopening the SQLite files continues on the right step, and the same decision sent again changes nothing.
- **C07, minimum:** a refused decision leaves no event; an answer lost on the way back is checked before sending again; a destination that stays down ends in failed after the limit; the same local key in T1 and T2 makes two effects.
- **C19:** [`examples/contracts`](../examples/contracts) installs it in two applications outside this workspace.

## What a pull request shows

1. The scenarios it serves, its implementation issue and the version or commit of what it depends on.
2. The public contract and a compiling example of an outside application configuring and using it; configuration kept apart from domain code.
3. The behavior of UI, service and worker, with failures, rollback and resume where they apply.
4. A matrix of hosts, databases and providers: implemented and tested, partial, not supported or planned, with the command that checked it.
5. Tests and steps anyone can repeat with synthetic data.
6. The rows of [APP-PARITY.md](APP-PARITY.md) and this page updated in the same change; two uses from different sectors once the package is consumable.
