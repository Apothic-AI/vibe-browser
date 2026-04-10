# VIBE.md Spec

> Status: Developer Preview 0.1
> Core idea: `VIBE.md` is the only required published artifact. Everything else is optional material described inside it.

## 1. What Vibe Is
Vibe is a convention for publishing one agent-facing Markdown file, `VIBE.md`, that tells a client:

- what this service is
- how a client should interact with the service
- what optional integrations exist
- what downloadable assets exist
- what each published asset is for

If the service exposes A2A, MCP, Agent Skills, OpenAPI, or other agent-relevant endpoints, `VIBE.md` is where they should be declared.

## 2. Discovery
Clients discover a Vibe-enabled service by attempting these paths in order:

1. `/.well-known/VIBE.md`
2. `/VIBE.md`

If neither path exists, the service is not Vibe-discoverable.

## 3. Valid `VIBE.md`
A valid `VIBE.md` MUST:

- be UTF-8 Markdown
- begin with one `#` heading naming the service
- include `## Service`
- include `## Instructions`
- include `## Integrations` if the service exposes any non-page integration or agent-relevant endpoint
- include `## Assets` if the service publishes direct asset URLs, a bundle, or both
- keep those sections in that order

Vibe does not require JSON. It does require a disciplined Markdown layout so different clients read the same file the same way.

## 4. Required Layout
The layout is the contract.

Required shape:

1. `# <service name>`
2. Optional opening paragraph
3. `## Service`
4. `## Instructions`
5. Optional `## Integrations`
6. Optional `## Experience`
7. Optional `## Assets`
8. Optional `## Notes`

Additional sections are allowed after these core sections, but they SHOULD NOT replace them.

## 5. Section Rules
### 5.1 `## Service`
`## Service` is required.

It SHOULD begin with short labeled lines or bullets using these labels:

- `Vibe version`
- `Service ID`
- `URL`
- `Summary`

Recommended additional labels:

- `Tags`
- `Audience`

Example:

````md
## Service
- Vibe version: 0.1.0
- Service ID: calendar.example
- URL: https://calendar.example
- Summary: Team scheduling and booking for adaptive clients.
- Tags: calendar, scheduling, booking
````

### 5.2 `## Instructions`
`## Instructions` is required.

This section tells the client how to behave around the service.

It SHOULD begin with these labels:

- `Client role`
- `Primary goal`

Recommended additional labels:

- `Entrypoints`
- `Use when`
- `Constraints`
- `Fallbacks`

Example:

````md
## Instructions
- Client role: adaptive_interface_agent
- Primary goal: Help the user search availability, create events, and book time slots.
- Entrypoints: search_events, create_event, book_time_slot
- Constraints:
  - Prefer brief confirmations before side effects.
  - Prefer local accessibility rules over publisher visual preference.
- Fallbacks:
  - If no live integration is available, surface the service homepage.
````

### 5.3 `## Integrations`
`## Integrations` is optional, but if the service exposes any agent-relevant surface, it SHOULD be present and list those surfaces explicitly.

Use these subsection names when relevant:

- `### A2A`
- `### MCP Servers`
- `### Skills`
- `### OpenAPI`
- `### AsyncAPI`
- `### Additional Endpoints`

Within each subsection, each integration SHOULD use a `#### <name>` heading followed by short labeled lines.

Example:

````md
## Integrations

### A2A
#### Primary agent
- Agent Card URL: https://calendar.example/.well-known/agent-card.json
- Role: Primary live task runtime
- Use when: The client wants delegated task execution or streaming updates.

### MCP Servers
#### calendar-tools
- Transport: http
- URL: https://calendar.example/mcp
- Purpose: Optional tool access for availability search and booking actions.
````

Rules:

- If an A2A agent exists and the client may need it, list it in `### A2A`.
- If an MCP server exists and the client may need it, list it in `### MCP Servers`.
- If Agent Skills packages exist and the client may need them, list them in `### Skills`.
- If OpenAPI or AsyncAPI documents exist and the client may need them, list them in `### OpenAPI` or `### AsyncAPI`.
- If any other endpoint may matter to the client agent, list it in `### Additional Endpoints`.

For skill entries:

- `Root` SHOULD point to the skill directory root.
- `Entry file` SHOULD normally be `SKILL.md`.
- the skill directory MAY contain optional `scripts/`, `references/`, and `assets/` directories

### 5.4 `## Experience`
`## Experience` is recommended when the service wants to describe intended interaction style or UI shape.

Recommended labels:

- `Summary`
- `Tone`
- `Key intents`
- `UI hints`
- `Preferred modalities`

### 5.5 `## Assets`
`## Assets` is required only when the service publishes assets outside the Markdown file itself.

It SHOULD contain these subsections:

- `### Bundle`
- `### Files`
- `### Individual Assets`

`### Bundle` SHOULD include:

- `URL`
- `Format`
- `Media type`
- `SHA256`

`### Files` MUST enumerate every file inside the bundle when `### Bundle` is present.

Each file SHOULD use a `#### <path>` heading followed by these labels:

- `Media type`
- `Purpose`
- `SHA256`
- `Required`

Recommended additional labels:

- `Description`
- `Locale`
- `Variants`
- `Alt`

`### Individual Assets` is optional.

Use it when the service publishes direct asset URLs outside the archive. Each direct asset SHOULD use a `#### <asset name>` heading followed by these labels:

- `URL`
- `Media type`
- `Purpose`

Recommended additional labels:

- `SHA256`
- `Required`
- `Description`
- `Alt`

Example:

````md
## Assets

### Bundle
- URL: https://calendar.example/vibe-assets.tar.gz
- Format: tar.gz
- Media type: application/gzip
- SHA256: sha256-archive-hash

### Files
#### images/logo.svg
- Media type: image/svg+xml
- Purpose: brand.logo
- SHA256: sha256-logo-hash
- Required: yes
- Description: Primary service logo.

#### copy/hero.md
- Media type: text/markdown
- Purpose: content.hero
- SHA256: sha256-hero-hash
- Required: no
- Description: Onboarding copy for the first-run experience.

### Individual Assets
#### primary-logo
- URL: https://calendar.example/assets/logo.svg
- Media type: image/svg+xml
- Purpose: brand.logo
- SHA256: sha256-direct-logo-hash
- Required: no
- Description: Direct logo URL for clients that skip the archive.
````

Both `### Bundle` and `### Individual Assets` MAY appear in the same `VIBE.md`.

### 5.6 `## Notes`
`## Notes` is optional.

Use it for brand guidance, interaction nuance, or non-essential caveats. Required instructions SHOULD NOT be hidden only in this section.

## 6. Authoring Rules
- Keep the required section names stable.
- Put important URLs, hashes, and identifiers on their own labeled lines.
- Prefer short bullets over long narrative paragraphs for actionable guidance.
- If a relevant optional section has nothing to say, omit it or write `None.` clearly.
- Absolute URLs are preferred for published services. Relative URLs are acceptable in local examples.

## 7. Runtime Surfaces
After reading `VIBE.md`, the client SHOULD:

1. Read `## Service` and `## Instructions` first.
2. Inspect `## Integrations` when present.
3. Use the listed integrations according to local policy and service intent.
4. Use `## Experience` and `## Assets` to shape rendering and asset loading.

Vibe defines no separate live runtime protocol.

## 8. Versioning
- Breaking changes to the required section layout require a major version bump.
- New optional sections or labels are minor-version changes.
- Editorial copy changes that preserve meaning are patch-level changes.

## 9. Design Rule
If a client agent may need to discover or rely on something, give it an explicit heading and labeled line in `VIBE.md`. Do not bury it in narrative prose.
