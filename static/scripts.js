let countdownInterval = null;
function initCountdown() {
    console.log("YESSS");
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
        if(minutes > 0) { timeStr += `${minutes}m `; }
        if(seconds > 0) { timeStr += `${seconds}s `; }
        timerElement.innerText = timeStr;
    }
    updateFn();
    countdownInterval = setInterval(updateFn, 1000);
}
document.addEventListener("htmx:after-swap", function(event) {
    console.log("IT RUNS", event);
    if (event.detail.target.id === "nav-user") {
        initCountdown();
    }
});
document.addEventListener("htmx:log", function(evt) {
    console.log("HTMX v2 Log:", evt.detail.msg, evt.detail.elt);
});
document.addEventListener("htmx:load", function(evt) {
    console.log("LOAD:", evt.detail.msg, evt.detail.elt);
});