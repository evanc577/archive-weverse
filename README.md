# download-weverse

Batach download Weverse posts and moments

![Terminal screencast](term.svg)

## Usage

1. Log onto Weverse in your web browser and click on some artist posts and moments.

2. Save `weverse.io` cookies in netscape format. Use [Export Cookies](https://addons.mozilla.org/en-US/firefox/addon/export-cookies-txt/) extension for Firefox, or [Get cookies.txt](https://chrome.google.com/webstore/detail/get-cookiestxt/bgaddhkoddajcdgocldbbfleckgcbcid?hl=en) extension for Chrome(ium).

2. Create a `config.toml` file in the same directory as the executable:

```toml
cookies_file = "cookies-weverse-io.txt" # path to your cookies file from step 2
max_connections = 20 # limit to 20 simultaneous network connections

[artists.dreamcatcher]
artist_download_path = "posts/dreamcatcher/artist"
moments_download_path = "posts/dreamcatcher/moments"
# Downloads all artist posts and moments for dreamcatcher

[artists.sunmi]
artist_download_path = "posts/sunmi/artist"
moments_download_path = "posts/sunmi/moments"
# Only download 10 most recent artist posts and moments for sunmi
recent_artist = 10
recent_moments = 10
```

4. Run the program.
