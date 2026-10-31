# Posts a release to Discord through a channel webhook: the same channels as Joystick Gobloon.
# Final releases use the DISCORD_WEBHOOK secret, pre-releases (alpha builds from dev)
# DISCORD_WEBHOOK_ALPHA. Without the matching secret nothing is posted.
param(
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $Repo,
    [string] $Sha = "main",
    [switch] $Prerelease
)

$webhook = if ($Prerelease) { $env:DISCORD_WEBHOOK_ALPHA } else { $env:DISCORD_WEBHOOK }
if (-not $webhook) {
    $name = if ($Prerelease) { "DISCORD_WEBHOOK_ALPHA" } else { "DISCORD_WEBHOOK" }
    Write-Host "No $name secret: not announcing $Version on Discord."
    exit 0
}

$page = "https://github.com/$Repo/releases/tag/v$Version"
$download = "https://github.com/$Repo/releases/download/v$Version"
$title = if ($Prerelease) { "Keyboard Gobloon $Version (alpha build)" } else { "Keyboard Gobloon $Version" }
$lines = @(
    $(if ($Prerelease) { "A new test build is out." } else { "A new release is out." }),
    "",
    "**Installer:** [KeyboardGobloon-$Version-setup.exe]($download/KeyboardGobloon-$Version-setup.exe)",
    "**Portable:** [KeyboardGobloon-$Version-portable.zip]($download/KeyboardGobloon-$Version-portable.zip)",
    "",
    "[What's new]($page)"
)
$embed = @{
    title       = $title
    url         = $page
    description = ($lines -join "`n")
    # Goblin green, as Joystick Gobloon; alpha builds in amber.
    color       = $(if ($Prerelease) { 0xE6A23C } else { 0x8CC82C })
}
$body = @{
    username = "Keyboard Gobloon"
    embeds   = @($embed)
}
# The app icon, once the repository has one.
$icon = "assets/icon/KeyboardGobloon-256.png"
if (Test-Path $icon) {
    $url = "https://raw.githubusercontent.com/$Repo/$Sha/$icon"
    $body.avatar_url = $url
    $embed.thumbnail = @{ url = $url }
}
$json = $body | ConvertTo-Json -Depth 6
Invoke-RestMethod -Method Post -Uri $webhook -ContentType "application/json; charset=utf-8" -Body ([System.Text.Encoding]::UTF8.GetBytes($json)) | Out-Null
Write-Host "Announced $title on Discord."
