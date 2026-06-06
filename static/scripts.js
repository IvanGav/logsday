// htmx.config.logAll = true;
let countdownInterval = null;
// configure marked.js
marked.use({
    tokenizer: {
        html(src) {
            const match = this.rules.block.html.exec(src);
            if (match) {
                return {
                    type: 'text',
                    raw: match[0],
                    text: match[0]
                };
            }
            return false;
        }
    },
    renderer: {
        image({ href, title, text }) {
            const isVideo = href.split('?')[0].endsWith('.mp4');
            if (isVideo) {
                const titleAttr = title ? `title="${title}"` : '';
                return `
                    <video controls class="log-video" ${titleAttr} style="max-width:100%; height:auto;">
                        <source src="${href}" type="video/mp4">
                        ${text || 'Your browser does not support the video tag.'}
                    </video>
                `.trim();
            }
            const titleAttr = title ? `title="${title}"` : '';
            return `<img src="${href}" alt="${text}" ${titleAttr}>`;
        }
    }
});

function initCountdown() {
    if (countdownInterval) {
        clearInterval(countdownInterval);
    }
    const timerElement = document.getElementById("countdown");
    if (!timerElement) return;
    const startTime = new Date().getTime();
    const targetTime = new Date(parseInt(timerElement.getAttribute("time-duration")) * 1000).getTime();
    const updateFn = () => {
        const now = new Date().getTime();
        const distance = targetTime - (now - startTime);
        if (distance < 0) {
            clearInterval(countdownInterval);
            htmx.trigger(document.getElementById("nav-user"), "load");
            return;
        }
        const days = Math.floor(distance / (1000 * 60 * 60 * 24));
        const hours = Math.floor((distance % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
        const minutes = Math.floor((distance % (1000 * 60 * 60)) / (1000 * 60));
        const seconds = Math.floor((distance % (1000 * 60)) / 1000);
        let timeStr = "";
        if(days > 0) { timeStr += `${days}d `; }
        if(hours > 0) { timeStr += `${hours}h `; }
        if(minutes > 0 && days == 0) { timeStr += `${minutes}m `; }
        if(seconds > 0 && days == 0 && hours == 0) { timeStr += `${seconds}s `; }
        timerElement.innerText = timeStr;
    }
    updateFn();
    countdownInterval = setInterval(updateFn, 1000);
}

function slug_from(name) {
    name = name.trim();
    let s = "";
    for(let i = 0; i < name.length; i++) {
        let c = name[i];
        if((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-' || c == '_') { s += c; }
        if((c >= 'A' && c <= 'Z')) { s += c.toLowerCase(); }
        if(c == ' ' || c == '\t') { s += '_'; }
    }
    if(s == "") { return "-"; }
    return s;
}

function autogenerateUsername(displayname) {
    let username = document.getElementById("username");
    if(!username) return;
    let usernameStr = slug_from(displayname.value);
    username.setAttribute("placeholder", usernameStr);
}

function autogenerateSlug(title) {
    let slug = document.getElementById("slug");
    if(!slug) return;
    let slugStr = slug_from(title.value);
    slug.setAttribute("placeholder", slugStr);
}

function replaceParticle(weeklen) {
    let particle = document.getElementById("logsday-weekday-particle");
    if(!particle) return;
    if(weeklen.value == "7") { particle.innerText = "on"; }
    else { particle.innerText = "after"; }
}

// newlog

function checkFileSize(input) {
    const limit = 10 * 1024 * 1024;
    if (input.files[0].size > limit) {
        alert("This file is too big! Please choose an image under 10 MB.");
        input.value = ""; // Clear the input
    }
}
function normalizeExtension(filename) {
    let split = filename.split('.');
    let ext = split[split.length-1].toLowerCase();
    switch(ext) {
        case "jpeg": { ext = "jpg"; }
        default: {}
    }
    split[split.length-1] = ext;
    return split.join(".");
}
function getUploadPath() {
    let uploadPath = window.location.pathname;
    let parts = uploadPath.split('/');
    // should be in either `/new/log/{projectSlug}` or `/edit/log/{projectSlug}/{logNum}`
    if(parts.length < 3) { console.error("Was expecting parts to be at least length 3, but it's", parts.length); return "sillylittleerrorbecauseuriiswrong"; }
    if(!parts[0] == "") { console.error("Was expecting parts[0] to be empty, but it's", parts[0]); return "sillylittleerrorbecauseuriiswrong"; }
    if(parts[1] == "new" && parts[2] == "log") {
        return `/new/media/${parts[3]}`;
    } else if(parts[1] == "edit" && parts[2] == "log") {
        return `/new/media/${parts[3]}/${parts[4]}`;
    } else {
        console.error(parts);
        return "sillylittleerrorbecauseuriiswrong";
    }
}
function getDeletePath(filenameToDelete) {
    let uploadPath = window.location.pathname;
    let parts = uploadPath.split('/');
    // should be in either `/new/log/{projectSlug}` or `/edit/log/{projectSlug}/{logNum}`
    if(parts.length < 3) { console.error("Was expecting parts to be at least length 3, but it's", parts.length); return "sillylittleerrorbecauseuriiswrong"; }
    if(!parts[0] == "") { console.error("Was expecting parts[0] to be empty, but it's", parts[0]); return "sillylittleerrorbecauseuriiswrong"; }
    if(parts[1] == "new" && parts[2] == "log") {
        return `/del/media/${parts[3]}/new/${filenameToDelete}`;
    } else if(parts[1] == "edit" && parts[2] == "log") {
        return `/del/media/${parts[3]}/${parts[4]}/${filenameToDelete}`;
    } else {
        console.error(parts);
        return "sillylittleerrorbecauseuriiswrong";
    }
}
function getUploadedFilesNewListItemDesc(filename, filesize, error = null) {
    let sizestr;
    if(filesize > (1024*1024)) {
        sizestr = (filesize/(1024*1024)).toFixed(2) + "MB";
    } else if(filesize > 1024) {
        sizestr = (filesize/1024).toFixed(2) + "KB";
    } else {
        sizestr = filesize + "B";
    }
    let button = document.createElement("button");
    let txt;
    let buttonTxt;
    if(!error) {
        txt = document.createTextNode("" + filename + " (" + sizestr + ")");
        button.setAttribute("onclick", "deleteMedia(this.parentElement.parentElement)");
        button.appendChild(document.createTextNode("delete"));
    } else {
        txt = document.createTextNode("Failed to upload '" + filename + "': " + error);
        button.setAttribute("onclick", "this.parentElement.parentElement.remove()");
        button.appendChild(document.createTextNode("confirm"));
    }
    let desc = document.createElement("p");
    desc.appendChild(txt);
    desc.appendChild(button);
    return desc;
}
function getUploadedFilesNewListItem(filename) {
    let li = document.createElement("div");
    li.classList.add("uploaded-file");
    li.setAttribute("uploadedfilename", filename);
    li.appendChild(document.createTextNode("Uploading " + filename + "..."));
    return li;
}
async function uploadAndInsertMedia(files) {
    let uploaded = document.getElementById("uploaded-files");
    let uploadPromises = [];
    for (let i = 0; i < files.length; i++) {
        let file = files[i];
        let filename = normalizeExtension(file.name);
        let li = uploaded.appendChild(getUploadedFilesNewListItem(filename));
        uploadPromises.push(uploadFile(li, file, filename));
    }
    try {
        let responses = await Promise.allSettled(uploadPromises);
        let insertEmbeds = "";
        for (let response of responses) {
            if (response.value.error == null) { insertEmbeds += "\n![](" + response.value.filepath + ")\n"; }
        }
        let markdownInput = document.getElementById("markdown-input");
        markdownInput.value += insertEmbeds;
        updatePreview(markdownInput);
    } catch (globalError) {
        console.error("Some promise got rejected when uploading files", globalError);
    }
}
async function deleteMedia(li) {
    let uploaded = document.getElementById("uploaded-files");
    let filename = li.getAttribute("uploadedfilename");
    li.innerText = 'Removing "' + filename + '"...';
    const response = await fetch(getDeletePath(encodeURIComponent(filename)), { method: "DELETE" });
    if (!response.ok) {
        console.error(error);
        li.firstChild.replaceWith(getUploadedFilesNewListItemDesc(filename, 0, error));
    } else {
        li.remove();
    }
}
async function uploadFile(li, file, filename) {
    const formData = new FormData();
    formData.append("file", file, filename);
    let uploadPath = getUploadPath();
    const response = await fetch(uploadPath, {
        // No 'Content-Type' header here as fetch handles it automatically
        method: "POST",
        body: formData
    });
    if (!response.ok) {
        li.firstChild.replaceWith(getUploadedFilesNewListItemDesc(filename, 0, "Something went wrong"));
    }
    let fileInfo = await response.json();
    if(fileInfo.error) {
        li.firstChild.replaceWith(getUploadedFilesNewListItemDesc(filename, 0, fileInfo.error));
    } else {
        li.firstChild.replaceWith(getUploadedFilesNewListItemDesc(filename, fileInfo.filesize));
    }
    return fileInfo;
}
async function renderCreatedOn(div) {
    const unixUtc = parseInt(div.getAttribute("unix-utc"), 10);
    if(isNaN(unixUtc)) return;
    try {
        const instant = Temporal.Instant.fromEpochMilliseconds(unixUtc * 1000);
        const dateString = instant.toLocaleString(navigator.language, {
            timeZone: 'UTC',
            month: 'short',
            day: 'numeric',
            year: 'numeric'
            // hour: 'numeric',
            // minute: '2-digit'
        });
        div.innerText = dateString;
    } catch(e) {
        console.log("Temporal formatting failed:", e);
    }
}

// .md editor
function updatePreview(markdownInput) {
    const rawMarkdown = markdownInput.value;
    const markdownPreview = document.getElementById('markdown-preview');
    markdownPreview.innerHTML = marked.parse(rawMarkdown);
}

function setupNewlogListeners() {
    const dropZone = document.getElementById('drop-zone');
    ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
        dropZone.addEventListener(eventName, e => e.preventDefault(), false);
    });
    ['dragenter', 'dragover'].forEach(eventName => {
        dropZone.addEventListener(eventName, () => dropZone.classList.add('drag-active'), false);
    });
    ['dragleave', 'drop'].forEach(eventName => {
        dropZone.addEventListener(eventName, () => dropZone.classList.remove('drag-active'), false);
    });
    dropZone.addEventListener('drop', (e) => {
        const dt = e.dataTransfer;
        const files = dt.files;
        if (files.length > 0) {
            uploadAndInsertMedia(files);
        }
    });
    updatePreview(document.getElementById("markdown-input")); // make sure that if there's any pre-existing text - it's rendered
}