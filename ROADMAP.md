# Marquee Roadmap

Marquee is a generic, local-first media server designed to identify, organize, and serve personal media libraries.

## V1 — Core functionality

### Media scanning and identification
- Recursive directory scanning
- Video file filtering
- Filename parsing through Hunch
- Extract titles, years, seasons and episode numbers
- Preserve original filenames and file paths
- Handle uncertain filename results without guessing
- Metadata identification through TMDb
- Embedded media metadata extraction
- Manual identification and correction
- Persistent identification (content-based file fingerprinting)
- Candidate retention for manual review

### Media server
- Serve the library over the local network
- Web interface
- Video playback (direct play, default audio track)

## V2 — Desktop application

- Native Qt + GStreamer player
- Full audio track switching (no remux, no browser limitations)
- Subtitle support with offset adjustment
- System tray, native file dialogs
- QWebEngineView wrapping existing web UI
- Fragmented MP4 live remux fallback for web UI (non-default tracks)

### Media organization
- Organize media into per-title folders
- Support index-in-place mode
- Handle multiple versions of the same media
- Handle files containing multiple episodes

### Filename parsing
- User-defined filename patterns for non-standard conventions (e.g., `ESx<number>` for Spanish-language episode numbering)
- Configurable parsing rules and naming conventions

### Library management
- Additional metadata providers
- Improved automatic identification
- Advanced library organization

## V3 — Discovery

- "Watch on" provider badges on details view (TMDb watch providers API)
- Browse tab: discover content by streaming service + region

## Future possibilities

### Library features
- Collections
- Multiple user profiles
- Remote access

### Maintenance
- Backup functionality
- Library monitoring

### Platform
- Mobile application
- Localization   
