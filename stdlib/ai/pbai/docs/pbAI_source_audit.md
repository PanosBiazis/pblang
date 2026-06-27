# pbAI Local Source Audit

This audit maps useful local material for pbAI without making old projects the pbAI
core. pbAI remains the pblang-native model/runtime in `ai/pbai`; external folders are
training, retrieval, evaluation, or design-reference sources.

## pbAI Project Shape

- `stdlib/pbai.sapp` - top-level assistant facade, routing, review skills, live memory,
  simple arithmetic, model status, and tool API.
- `stdlib/pbai_lm.sapp` - pblang-native char-level Nano LM, curriculum corpus, live
  corpus, training, and generation.
- `stdlib/pbai_tokenizer.sapp` - pblang-native tokenizer contract.
- `stdlib/pbai_brain.sapp` - perception, modalities, neural cognitive core, planning,
  critique, verification, reflection, and agent hierarchy.
- `stdlib/pbai_ingest.sapp` - bounded local file/folder ingestion into live learning.
- `stdlib/pbai_tools.sapp` - local capability/source map.
- `tools/pbai_server.sapp` - pblang-only local website/API server.
- `tools/pbai_api.sapp` - one-shot JSON bridge for scripts/tools.
- `tools/pbai_feed.sapp` - command-line feed modes for lessons, paths, AAP quick feed,
  mounted drives, and phone/MTP sources.
- `website/` - live test console, learning controls, theme mode, chat, and 3D model map.
- `tests/semantics/` - regression contract for assistant, LM, tokenizer, brain, assets.

## Highest-Value AAP Sources

- `pl-dev/pblang` - pblang compiler, stdlib, docs, IDE, self-hosted frontend, lint/type
  checker, JIT/profiler/debugger references. This is the most important source for
  pblang-specific intelligence.
- `tools/rust/pb-mcp-server` - MCP tool catalog, code index, multi-agent bus, consensus,
  reasoning, OCR/media/neural/data tools, security gates, and gateway architecture.
- `datasets/data` - TinyStories, books, PG19, BookCorpus-clean, combined books text,
  GloVe embeddings, tokenizer assets, and Arrow corpora.
- `datasets/models/ai_model.joblib` - classical ML baseline/reference artifact.
- `training/*` - many small language examples across HTML, C++, Kotlin, Bash, Batch,
  TypeScript, Arduino, Android, PHP, and C#.
- `cyber-security/*`, `data-science/*`, `robotics/*`, `quantum/*`, `system-tools/*` -
  domain-specific reference/evaluation material.
- `web/job-tracker` - app docs and API/feature documentation.

## AAP AI Sources

- `ai/max` - tokenizer/config/artifacts, training scripts, Pascal neural/text processor
  references, and model metadata. Useful as baseline/reference only.
- `ai/aipy`, `ai/takisbot` - Python AI/tokenizer/training scripts and examples.
- `ai/Sp_AIModel` - Pascal model/text processor reference.
- `ai/AI`, `ai/Delphi_AI` - repeated Delphi/C# assistant prototypes, chat history,
  Google Drive integration, file parsing, RAG, and question-template ideas.
- `ai/WEB/FinIS` - financial/reporting web system, auth, services, technical analysis,
  localization, attachments, and reporting docs.
- `ai/WEB/Sales` - sales/customer search app, data model, importers, auth/security docs.
- `ai/WEB/Specisoft.*` - financial migration, data repositories, reporting abstractions,
  report renderers, and legacy-to-web modernization references.

## Sibling Projects Folder

The sibling projects folder beside AAP can contain useful external references:

- `Sp_AIModel`, `aipy`, `max`, `chatbot` - older AI/model/chat experiments.
- `progrCS/RealAIVisualizer`, `progrCS/app2`, `simServer` - visualization/server ideas.
- `WordDetect`, `wd`, `wdps` - text/word detection app references.
- `sw/*` - large Delphi/financial/business systems, SQL, reporting, graphing, and legacy
  modernization material.
- Laravel, Django, Blazor, C# calculator, Delphi examples, SFML/OpenGL/Pong/game folders -
  cross-language examples and UI/app architecture references.

## Connected Drive And Phone Sources

Drive/feed roots are intentionally configurable instead of hardcoded in the repository.
Set `PBAI_FEED_ROOTS` to a semicolon-separated list of local folders, mounted drives, or
MTP/device paths:

```bash
export PBAI_FEED_ROOTS="/path/to/data;/media/me/Drive;/run/user/1000/gvfs/mtp:host=Phone/Internal storage"
```

When `PBAI_FEED_ROOTS` is unset, `tools/pbai_feed.sapp` uses safe project-relative
defaults plus `/mnt/win`.

Use bounded commands:

```bash
pblang run tools/pbai_feed.sapp --allow-all -- drives_survey
pblang run tools/pbai_feed.sapp --allow-all -- drives 10 2
pblang run tools/pbai_feed.sapp --allow-all -- path "../../ai/WEB/FinIS" 40 3
pblang run tools/pbai_feed.sapp --allow-all -- path "/path/to/curated/project" 40 3
```

## Exclusions

Do not blindly ingest secrets or generated noise:

- `credentials.json`, `credentialsgd.json`, `appsettings*.json`
- `.env`, `.ssh`, `id_ed25519*`, `openrouter.txt`
- filenames containing password, secret, token, key
- `.git`, `target`, `node_modules`, `.venv`, caches, build outputs, installers, ISOs,
  executables, and huge binaries

Binary/media files should be learned as metadata until pbAI has a modality-specific
parser. Text/code/docs should be learned as bounded chunks.
