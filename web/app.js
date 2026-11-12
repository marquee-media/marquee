const mediaGrid = document.getElementById("media-grid");
const message = document.getElementById("message");
const searchInput = document.getElementById("search-input");
const sortSelect = document.getElementById("sort-select");
const libraryCount = document.getElementById("library-count");
const pendingCount = document.getElementById("pending-count");
const pendingList = document.getElementById("pending-list");
const libraryToolbar = document.getElementById("library-toolbar");
const mediaDetails = document.getElementById("media-details");

let library = [];
let pending = [];
let currentView = "movies";

const IMAGE_BASE = "https://image.tmdb.org/t/p/w500";

async function fetchJson(url) {
    const response = await fetch(url);

    if (!response.ok) {
        throw new Error(`Request failed: ${response.status}`);
    }

    return response.json();
}

async function loadLibrary() {
    showMessage("Loading your library...");

    try {
        const [libraryData, pendingData] = await Promise.all([
            fetchJson("/api/library"),
                                                             fetchJson("/api/pending")
        ]);

        library = libraryData;
        pending = pendingData;

        pendingCount.textContent = pending.length;
        updateLibraryCount();
        render();
    } catch (error) {
        console.error("Failed to load library:", error);
        showMessage(
            "Couldn't connect to Marquee. Make sure the server is running and try refreshing.",
            true
        );
        libraryCount.textContent = "Connection failed";
    }
}

function showMessage(text, isError = false) {
    message.textContent = text;
    message.hidden = false;
    message.classList.toggle("error", isError);
}

function hideMessage() {
    message.hidden = true;
    message.classList.remove("error");
}

function updateLibraryCount() {
    const uniqueMedia = new Map();

    for (const item of library) {
        const key = `${item.media_type}:${item.tmdb_id}`;
        uniqueMedia.set(key, item);
    }

    const count = uniqueMedia.size;
    libraryCount.textContent =
    `${count} title${count === 1 ? "" : "s"} in your library`;
}

function getVisibleLibrary() {
    const query = searchInput.value.trim().toLowerCase();

    const results = library.filter(item => {
        const matchesType =
        currentView === "movies"
        ? item.media_type === "movie"
        : item.media_type === "tv";

        const matchesSearch =
        !query ||
        (item.title || "").toLowerCase().includes(query);

        return matchesType && matchesSearch;
    });

    const unique = new Map();

    for (const item of results) {
        const key = `${item.media_type}:${item.tmdb_id}`;
        if (!unique.has(key)) {
            unique.set(key, item);
        }
    }

    const sorted = [...unique.values()];
    const sort = sortSelect.value;

    sorted.sort((a, b) => {
        if (sort === "year-desc") {
            return (b.year || 0) - (a.year || 0);
        }

        if (sort === "year-asc") {
            return (a.year || 0) - (b.year || 0);
        }

        return (a.title || "").localeCompare(b.title || "");
    });

    return sorted;
}

function createMediaCard(item) {
    const card = document.createElement("button");
    card.className = "media-card";
    card.type = "button";
    card.addEventListener("click", () => showMediaDetails(item));

    const poster = document.createElement("div");
    poster.className = "poster";

    if (item.poster_path) {
        const image = document.createElement("img");
        image.src = `${IMAGE_BASE}${item.poster_path}`;
        image.alt = `${item.title || "Media"} poster`;
        image.loading = "lazy";

        image.addEventListener("error", () => {
            poster.replaceChildren(createPlaceholder(item.title));
        });

        poster.appendChild(image);
    } else {
        poster.appendChild(createPlaceholder(item.title));
    }

    const title = document.createElement("h2");
    title.className = "media-title";
    title.textContent = item.title || "Unknown title";

    const year = document.createElement("p");
    year.className = "media-year";
    year.textContent = item.year || "Year unknown";

    card.append(poster, title, year);

    return card;
}

function createPlaceholder(title) {
    const placeholder = document.createElement("div");
    placeholder.className = "poster-placeholder";
    placeholder.textContent = title || "No poster available";
    return placeholder;
}

function showMediaDetails(item) {
    mediaGrid.hidden = true;
    libraryToolbar.hidden = true;
    pendingList.hidden = true;
    mediaDetails.hidden = false;
    hideMessage();

    mediaDetails.replaceChildren();

    const backButton = document.createElement("button");
    backButton.className = "details-back";
    backButton.textContent = "← Back to library";
    backButton.addEventListener("click", () => {
        mediaDetails.hidden = true;
        mediaGrid.hidden = false;
        libraryToolbar.hidden = false;
    });

    const poster = document.createElement("div");
    poster.className = "details-poster";

    if (item.poster_path) {
        const image = document.createElement("img");
        image.src = `${IMAGE_BASE}${item.poster_path}`;
        image.alt = `${item.title || "Media"} poster`;
        poster.appendChild(image);
    } else {
        poster.appendChild(createPlaceholder(item.title));
    }

    const content = document.createElement("div");
    content.className = "details-content";

    const title = document.createElement("h2");
    title.textContent = item.title || "Unknown title";

    const meta = document.createElement("p");
    meta.className = "details-meta";
    meta.textContent = [
        item.year || "Year unknown",
        item.media_type === "movie" ? "Movie" : "TV show"
    ].join(" · ");

    const overview = document.createElement("p");
    overview.className = "details-overview";
    overview.textContent = item.overview || "No overview available.";


    const player = document.createElement("video");
    player.className = "details-player";
    player.controls = true;
    player.preload = "metadata";
    player.src = `/stream/${encodeURIComponent(item.file_key)}`;

    const location = document.createElement("p");
    location.className = "details-location";

    const primaryLocation = item.locations.find(location => location.is_primary)
    || item.locations[0];

    location.textContent = primaryLocation
    ? `File location: ${primaryLocation.path}`
    : "No file location available.";

    content.append(
        title,
        meta,
        overview,
        player,
        location
    );

    mediaDetails.append(backButton, poster, content);
}

function renderLibrary() {
    pendingList.hidden = true;
    libraryToolbar.hidden = false;
    mediaGrid.hidden = false;

    const items = getVisibleLibrary();

    mediaGrid.replaceChildren();

    if (items.length === 0) {
        showMessage(
            searchInput.value.trim()
            ? "No titles match your search."
            : "No media found in this section yet."
        );
        return;
    }

    hideMessage();

    for (const item of items) {
        mediaGrid.appendChild(createMediaCard(item));
    }
}

function renderPending() {
    libraryToolbar.hidden = true;
    mediaGrid.hidden = true;
    pendingList.hidden = false;

    pendingList.replaceChildren();

    if (pending.length === 0) {
        showMessage("Everything is identified. No items need review.");
        return;
    }

    hideMessage();

    for (const item of pending) {
        const entry = document.createElement("article");
        entry.className = "pending-item";

        const title = document.createElement("h3");
        title.textContent = item.original_filename;

        const path = document.createElement("p");
        path.textContent = item.original_path;

        const candidateInfo = document.createElement("p");
        candidateInfo.textContent =
        `${item.candidates.length} identification candidate` +
        (item.candidates.length === 1 ? "" : "s");

        entry.append(title, path, candidateInfo);
        pendingList.appendChild(entry);
    }
}

function render() {
    mediaDetails.hidden = true;

    if (currentView === "pending") {
        renderPending();
    } else {
        renderLibrary();
    }
}

function switchView(view) {
    currentView = view;

    const isPending = view === "pending";
    const isSeries = view === "series";

    document.querySelectorAll(".nav-item").forEach(button => {
        button.classList.toggle(
            "active",
            button.dataset.view === view
        );
    });

    document.getElementById("page-name").textContent =
    isPending ? "Needs review" : isSeries ? "TV Shows" : "Movies";

    document.getElementById("page-title").textContent =
    isPending ? "Needs review" : isSeries ? "TV Shows" : "Movies";

    document.getElementById("page-description").textContent =
    isPending
    ? "Review media that Marquee couldn't identify automatically."
    : isSeries
    ? "Your television collection, all in one place."
    : "Your movie collection, all in one place.";

    searchInput.value = "";
    render();
}

document.querySelectorAll(".nav-item").forEach(button => {
    button.addEventListener("click", () => {
        switchView(button.dataset.view);
    });
});

searchInput.addEventListener("input", render);
sortSelect.addEventListener("change", render);

loadLibrary();

