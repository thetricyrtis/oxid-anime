# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-10-31

### Changed

- Database and cache files now live in OS-standard directories
  (`~/.local/share/oxid-anime/` and `~/.cache/oxid-anime/` on Linux).
- Translations are embedded into the binary via `include_str!`; the `locales/`
  directory is no longer required at runtime.

### Added

- Automatic offline-database download on first launch when the `anime` table is
  empty.
- Status message on the top bar (right side) while the database is being
  downloaded or extracted.

## [0.2.0] - 2026-10-26

Early-access release. Search (offline/online/hybrid), SQLite-backed library,
AniList integration, poster rendering, TR/EN localization. Detailed per-change
log was not maintained before this release.

