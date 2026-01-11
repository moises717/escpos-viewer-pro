# Releasing (escpos_viewer)

Este repo usa **release-plz** para automatizar versiones + tags.

Flujo:

1) Tú haces commits normales a `main` (idealmente con Conventional Commits: `feat:`, `fix:`, etc.).
2) GitHub Actions crea/actualiza automáticamente un **Release PR** con:
	 - bump de versión en `Cargo.toml`
	 - actualización de `CHANGELOG.md`
3) Cuando haces merge del Release PR, release-plz crea el tag `vX.Y.Z`.
4) El workflow [​.github/workflows/windows-release.yml](.github/workflows/windows-release.yml) se dispara con ese tag y publica el Release con los artefactos.

## Requisitos en GitHub

- En el repo: Settings → Actions → General → **Workflow permissions** → habilitar
	“Read and write permissions” y permitir crear PRs.

## Auto-merge (opcional)

Este repo incluye el workflow [​.github/workflows/auto-merge-release-plz.yml](.github/workflows/auto-merge-release-plz.yml) que habilita
automáticamente el **auto-merge** para PRs de release-plz (ramas `release-plz-*`).

Para que funcione, habilita en GitHub:
- Settings → General → Pull Requests → **Allow auto-merge**

Luego, cuando el CI esté en verde, GitHub mergea el Release PR sin intervención.

## Notas

- El instalador toma la versión desde el tag (CI pasa `/DMyAppVersion=...` a Inno Setup), así que no tienes que editar `setup.iss` manualmente.
- Si no quieres que haya Release PR por cada commit, podemos configurar `release_commits` para que solo dispare con `feat:`/`fix:`.
