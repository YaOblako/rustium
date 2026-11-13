$urls = @(
    "https://upload.wikimedia.org/wikipedia/commons/2/2c/Rotating_earth_%28large%29.gif",
    "https://upload.wikimedia.org/wikipedia/commons/e/ec/Fire_breathing_1.gif",
    "https://upload.wikimedia.org/wikipedia/commons/7/70/Star_trails_animation.gif",
    "https://upload.wikimedia.org/wikipedia/commons/1/1a/Animated-water-fall.gif",
    "https://upload.wikimedia.org/wikipedia/commons/6/6a/Rain_window.gif",
    "https://upload.wikimedia.org/wikipedia/commons/9/91/Wave_animation.gif"
)

foreach ($url in $urls) {
    try {
        $req = [System.Net.HttpWebRequest]::Create($url)
        $req.Method = "HEAD"
        $req.UserAgent = "Mozilla/5.0"
        $req.Timeout = 5000
        $resp = $req.GetResponse()
        Write-Output "$([int]$resp.StatusCode) : $url"
        $resp.Close()
    } catch {
        Write-Output "ERR ($($_.Exception.Message)) : $url"
    }
}
