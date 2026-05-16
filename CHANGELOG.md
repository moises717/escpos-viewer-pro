# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.6.0](https://github.com/moises717/escpos-viewer-pro/releases/tag/v1.6.0) - 2026-05-16

### Added

- *(printer)* implement dynamic online/offline status for better queue management
- agregar registro de habilidades y estándares para el proyecto
- *(printer)* fix localized driver detection and improve setup robustness
- *(printer)* mejorar la instalación y desinstalación de impresoras en Windows
- *(parser)* implementar posicionamiento absoluto y gráficos legacy
- improve ESC/POS alignment and text scaling accuracy
- add realistic thermal paper visual effects and custom font support
- mejorar documentación sobre la instalación de la impresora virtual en el README
- actualizar imágenes de vista previa en el README
- actualizar imagen de vista previa en el README a un GIF
- agregar imagen de vista previa al README
- agregar documentación y capturas de pantalla al README
- add support for ESC t command to change code page in ESC/POS parsing
- enhance ESC/POS barcode handling and text decoding
- refactor job management and enhance simulation handling in EscPosViewer
- add barcode/HRI parameter handling and improve ESC/POS command parsing
- agregar configuración de automatización de versiones y changelog
- add single instance support to prevent multiple instances and focus existing window on Windows
- add MIT License file
- add ESC/POS printer support with TCP capture and system tray integration

### Fixed

- move version detection before build step in release workflow
- add  automatic release tags
- *(ci)* skip windows release without tag

### Other

- *(release)* bump version to 1.6.0 and implement dynamic printer status
- *(ci)* run windows release after release-plz
- *(ci)* use PAT for release-plz tags
- enable Windows subsystem for GUI in release builds
- *(ci)* auto-merge release-plz PRs
- release v1.0.0
- update Cargo.lock
- bump version to 1.0.0
- release v0.1.0

## [1.5.0](https://github.com/moises717/escpos-viewer-pro/releases/tag/v1.5.0) - 2026-05-16

### Added

- agregar registro de habilidades y estándares para el proyecto
- *(printer)* fix localized driver detection and improve setup robustness
- *(printer)* mejorar la instalación y desinstalación de impresoras en Windows
- *(parser)* implementar posicionamiento absoluto y gráficos legacy
- improve ESC/POS alignment and text scaling accuracy
- add realistic thermal paper visual effects and custom font support
- mejorar documentación sobre la instalación de la impresora virtual en el README
- actualizar imágenes de vista previa en el README
- actualizar imagen de vista previa en el README a un GIF
- agregar imagen de vista previa al README
- agregar documentación y capturas de pantalla al README
- add support for ESC t command to change code page in ESC/POS parsing
- enhance ESC/POS barcode handling and text decoding
- refactor job management and enhance simulation handling in EscPosViewer
- add barcode/HRI parameter handling and improve ESC/POS command parsing
- agregar configuración de automatización de versiones y changelog
- add single instance support to prevent multiple instances and focus existing window on Windows
- add MIT License file
- add ESC/POS printer support with TCP capture and system tray integration

### Fixed

- move version detection before build step in release workflow
- add  automatic release tags
- *(ci)* skip windows release without tag

### Other

- *(ci)* run windows release after release-plz
- *(ci)* use PAT for release-plz tags
- enable Windows subsystem for GUI in release builds
- *(ci)* auto-merge release-plz PRs
- release v1.0.0
- update Cargo.lock
- bump version to 1.0.0
- release v0.1.0

## [1.0.0](https://github.com/moises717/escpos-viewer-pro/releases/tag/v1.0.0) - 2026-01-11

### Added

- agregar configuración de automatización de versiones y changelog
- add single instance support to prevent multiple instances and focus existing window on Windows
- add MIT License file
- add ESC/POS printer support with TCP capture and system tray integration

### Other

- update Cargo.lock
- bump version to 1.0.0
- release v0.1.0

## [0.1.0](https://github.com/moises717/escpos-viewer-pro/releases/tag/v0.1.0) - 2026-01-11

### Added

- agregar configuración de automatización de versiones y changelog
- add single instance support to prevent multiple instances and focus existing window on Windows
- add MIT License file
- add ESC/POS printer support with TCP capture and system tray integration
