
# Marquee Roadmap

Marquee is a generic, local-first media server designed to identify, organize, and serve personal media libraries.

## V1 — Core functionality

### Media scanning and identification
- [x] Recursive directory scanning
- [x] Video file filtering
- [x] Filename parsing through Hunch
- [x] Extract titles, years, seasons and episode numbers
- [x] Preserve original filenames and file paths
- [x] Handle uncertain filename results without guessing
- [ ] Metadata identification through TMDb
- [ ] Embedded media metadata extraction
- [ ] Manual identification and correction

### Media organization
- [ ] Organize media into per-title folders
- [ ] Support index-in-place mode
- [ ] Preserve original files and avoid destructive operations
- [ ] Handle multiple versions of the same media
- [ ] Handle files containing multiple episodes

### Media server
- [ ] Serve the library over the local network
- [ ] Web interface
- [ ] Video playback
- [ ] Subtitle support

## V2 — Extensibility and improvements

### Filename parsing
- [ ] User-defined filename patterns for non-standard conventions (e.g., `ESx<number>` for Spanish-language episode numbering).
- [ ] Configurable parsing rules and naming conventions

### Library management
- [ ] Additional metadata providers
- [ ] Improved automatic identification
- [ ] Advanced library organization

## Future possibilities

Features not currently committed to a specific release:
- Collections
- Achievements
- Multiple user profiles
- Remote access
- Backup functionality
- Library monitoring
- Localization
- Mobile application
- Standalone desktop application
