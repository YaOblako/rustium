(function() {
    console.log("[Rustium Extension] Rustium Assistant loaded for: " + window.location.href);

    if (window.chrome && window.chrome.storage && window.chrome.storage.local) {
        window.chrome.storage.local.get("visit_count", function(res) {
            const count = (res.visit_count || 0) + 1;
            window.chrome.storage.local.set({ visit_count: count });
        });
    }

    if (!document.getElementById("rustium-page-assistant-badge")) {
        const badge = document.createElement("div");
        badge.id = "rustium-page-assistant-badge";
        badge.textContent = "🛡️ Rustium Protected";
        (document.body || document.documentElement).appendChild(badge);

        setTimeout(() => {
            if (badge.parentNode) {
                badge.style.opacity = "0";
                setTimeout(() => badge.remove(), 400);
            }
        }, 3000);
    }
})();
