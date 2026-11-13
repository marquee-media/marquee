# Marquee Roadmap

Marquee is a generic, local-first media server and media player designed to identify, organize, and serve personal media libraries.

The primary goal is to provide a reliable, easy-to-use application for browsing and playing personal media collections.

---

## V1 — Core functionality

### Media scanning and identification

- Recursive directory scanning
- Video file filtering
- Filename parsing through Hunch
- Extract titles, years, seasons, and episode numbers
- Preserve original filenames and file paths
- Handle uncertain filename results without guessing
- Metadata identification through TMDb
- Embedded media metadata extraction
- Manual identification and correction
- Persistent identification through content-based file fingerprinting
- Candidate retention for manual review

### Media server

- Serve the library over the local network
- Web interface
- Video playback

---

## V2 — Desktop application

The primary goal of V2 is to deliver a usable native desktop media application with reliable local playback.

### Desktop application

- Native Qt/QML desktop application
- Embedded backend integration
- System tray integration
- Native file dialogs
- Application settings
- Persistent library configuration

### Library browsing and management

- Browse movies and TV shows
- Display posters and metadata
- Movie and TV show details
- Add and remove library folders
- Initial library setup
- Library scanning from the desktop application
- Search, sorting, and basic filtering
- Handle missing or unavailable media files

### Playback

- Native GStreamer playback
- Direct playback without unnecessary transcoding
- Playback controls, seeking, and volume
- Audio track selection
- Subtitle selection
- Subtitle offset adjustment
- Remember playback position
- Resume playback
- Graceful playback error handling
- Support for common media formats

---

## V2.x — Improvements

### V2.1 — Playback improvements

- Improved format compatibility
- Better playback error handling
- Improved audio and subtitle handling
- Playback performance improvements
- Additional playback options

### V2.2 — Library improvements

- Multiple versions of the same media
- Support for files containing multiple episodes
- Improved library organization
- Index-in-place and organized-library modes
- Improved manual identification and correction

### V2.3 — Customization and metadata

- User-defined filename patterns
- Configurable parsing rules and naming conventions
- Additional metadata providers
- Improved automatic identification
- Advanced library organization

### Future desktop enhancements

- More detailed movie and TV show pages
- Cast and character information
- Trailers and previews
- Improved artwork and animated backgrounds
- Collections
- Advanced sorting and filtering
- Interface customization

---

## V3 — Discovery

### Streaming availability

- Streaming provider badges on details pages
- TMDb watch providers integration
- Browse content by streaming service
- Region-specific streaming availability

---

## Future possibilities

### Library features

- Multiple user profiles
- Remote access
- Library monitoring
- Backup and restore functionality
- Advanced collection management

### Platform

- Cross-platform desktop support
- Mobile application
- Localization

---

## Development principles

- **Reliability first:** Core functionality comes before optional features.
- **Playback first:** Playing media should be simple and reliable.
- **Local-first:** Personal media libraries remain under the user's control.
- **Generic:** Never assume a particular folder structure or naming convention.
- **Performance:** Avoid unnecessary processing, transcoding, and background work.
- **Maintainability:** Prefer simple, reusable implementations over unnecessary complexity.
- **Incremental development:** Build and test core functionality before expanding the feature set.
