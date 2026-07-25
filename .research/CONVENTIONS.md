# Research Conventions

Grounding, citation, and authority rules for `.research/` artifacts in this
repository. Aligned with `.work/CONVENTIONS.md` and `AGENTS.md`.

## Grounding floor

- Fetch every grounding source during the current engagement. Model memory may
  guide a search but may never serve as a citation, URL, date, or other
  bibliographic fact.
- Attest before synthesis: every cited detail first lands in
  `.research/attestations/<handle>.md` (frontmatter: `source_handle`,
  `fetched`, `source_title`, `source_url`) under `## Attested details`, with
  source-internal anchors.
- Local repository sources (e.g. a neighboring checkout) may be attested with
  a `file://` URL plus the commit or revision observed at fetch time.
- Seek disconfirming evidence before each load-bearing conclusion; every brief
  carries a `## Disconfirming evidence` section.

## Citation syntax

Cite attested details as `[handle]{N}` in briefs, where `N` is the numbered
attested detail. Label cross-source or beyond-source composition as inference.
Briefs live in `.research/briefs/<id>.md`; `bibliography.yaml` maps handles to
sources.

## Authority boundary

Research informs work; it does not decide it. Work items, plans, and prior
syntheses are analytical lenses, not attestations. Never rewrite an
attestation to support a downstream decision. `.knowledge/index.json` is
generated discovery metadata with no independent authority.

## Privacy

Never fetch, attest, synthesize, or index PII, PHI, credentials, session
material, or other sensitive data through an LLM-connected surface. Stop and
ask for redaction or an approved non-LLM path when material may contain it.
