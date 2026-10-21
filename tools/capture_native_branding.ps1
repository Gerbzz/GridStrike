[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'This capture runs only on an approved hosted Windows Actions runner.'
}
if ($env:GITHUB_REPOSITORY -ne 'Gerbzz/GridStrike') { throw 'Unexpected repository.' }
if (-not $env:GH_TOKEN) { throw 'A read-only Actions token is required.' }

$repo = 'Gerbzz/GridStrike'
$buildRun = 39574583998L
$artifactId = 12334213850L
$buildHead = '18859694a029ff25ecdd8ccaeebd870c7b277734'
$baseTree = 'e1ffa03a62b1ab26313b05929c25be3815e372a8'
$zipHash = 'e79176211ea8e0b12c0d72fc9dae564c396fa1f32b79736601d6cf6201b3dddc'
$work = Join-Path $env:RUNNER_TEMP ('gridstrike-capture-' + [guid]::NewGuid().ToString('N'))
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $work, $output | Out-Null

# The API client does not follow redirects with its Authorization header.
$handler = [Net.Http.HttpClientHandler]::new()
$handler.AllowAutoRedirect = $false
$api = [Net.Http.HttpClient]::new($handler)
$api.DefaultRequestHeaders.UserAgent.ParseAdd('GridStrike-native-branding-capture')
$api.DefaultRequestHeaders.Authorization = [Net.Http.Headers.AuthenticationHeaderValue]::new('Bearer', $env:GH_TOKEN)
$api.DefaultRequestHeaders.Add('X-GitHub-Api-Version', '2022-11-28')
function Read-GitHubJson([string]$Path) {
    $json = $api.GetStringAsync("https://api.github.com/repos/$repo/$Path").GetAwaiter().GetResult()
    return $json | ConvertFrom-Json -Depth 100
}
try {
    $run = Read-GitHubJson "actions/runs/$buildRun"
    if ($run.head_sha -ne $buildHead -or $run.status -ne 'completed' -or $run.conclusion -ne 'success' -or
        $run.path -ne '.github/workflows/geometric-branding.yml') { throw 'Retained build identity or result changed.' }
    $artifacts = Read-GitHubJson "actions/runs/$buildRun/artifacts"
    $matches = @($artifacts.artifacts | Where-Object id -EQ $artifactId)
    if ($matches.Count -ne 1) { throw 'Exact retained artifact is missing.' }
    $artifact = $matches[0]
    if ($artifact.expired -or $artifact.name -ne 'gridstrike-geometric-branding-native' -or
        $artifact.workflow_run.head_sha -ne $buildHead -or $artifact.digest -ne "sha256:$zipHash") {
        throw 'Retained artifact metadata does not match its approved digest and source.'
    }
    $source = Read-GitHubJson "git/commits/$buildHead"
    $main = Read-GitHubJson 'branches/main'
    if ($source.tree.sha -ne $baseTree -or $main.commit.commit.tree.sha -ne $baseTree) {
        throw 'The retained executable source tree no longer exactly matches current main.'
    }
    $event = Get-Content -LiteralPath $env:GITHUB_EVENT_PATH -Raw | ConvertFrom-Json -Depth 100
    if ($env:GITHUB_EVENT_NAME -eq 'pull_request') {
        $base = Read-GitHubJson "git/commits/$($event.pull_request.base.sha)"
        if ($base.tree.sha -ne $baseTree) { throw 'The PR base tree differs from the retained build.' }
    }
    # Only capture infrastructure, published screenshots and provenance may differ.
    & git diff --exit-code $buildHead HEAD -- . ':(exclude).github/workflows/native-branding-capture.yml' ':(exclude)tools/capture_native_branding.ps1' ':(exclude)assets/screenshots/**' ':(exclude)docs/SCREENSHOT_PROVENANCE.md'
    if ($LASTEXITCODE -ne 0) { throw 'Application, artwork or verifier source differs from the retained build.' }
    $response = $api.GetAsync("https://api.github.com/repos/$repo/actions/artifacts/$artifactId/zip").GetAwaiter().GetResult()
    try {
        if ([int]$response.StatusCode -ne 302) { throw 'Expected the official artifact download redirect.' }
        $location = $response.Headers.Location
        if (-not $location.IsAbsoluteUri -or $location.Scheme -ne 'https') { throw 'Invalid artifact download URL.' }
        $download = [Net.Http.HttpClient]::new()
        try {
            $bytes = $download.GetByteArrayAsync($location).GetAwaiter().GetResult()
            if ($bytes.Length -gt 32MB) { throw 'Unexpected artifact size.' }
            $zip = Join-Path $work 'retained.zip'
            [IO.File]::WriteAllBytes($zip, $bytes)
        } finally { $download.Dispose() }
    } finally { $response.Dispose() }
} finally { $api.Dispose(); $handler.Dispose() }
if ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant() -ne $zipHash) { throw 'Artifact ZIP digest mismatch.' }
# No token is inherited by the native app or written to the capture proof.
$env:GH_TOKEN = $null
$env:GITHUB_TOKEN = $null

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [IO.Compression.ZipFile]::OpenRead($zip)
try {
    $entries = @($archive.Entries | Where-Object { ($_.FullName -replace '\\', '/') -match '(^|/)gridstrike\.exe$' })
    if ($entries.Count -ne 1 -or $entries[0].Length -gt 200MB) { throw 'Expected one bounded retained GridStrike executable.' }
    $exe = Join-Path $work 'gridstrike.exe'
    $inputStream = $entries[0].Open()
    $outputStream = [IO.File]::Create($exe)
    try { $inputStream.CopyTo($outputStream) } finally { $inputStream.Dispose(); $outputStream.Dispose() }
} finally { $archive.Dispose() }

& python tools/test_verify_geometric_identity.py
if ($LASTEXITCODE -ne 0) { throw 'Icon verifier negative controls failed.' }
$iconJson = & python tools/verify_geometric_identity.py --svg assets/branding/GridStrike-geometric.svg --png assets/branding/GridStrike-geometric.png --ico assets/app-icon/gridstrike-geometric.ico --original assets/branding/GridStrike.png --original-sha256 4b1ca5b24064fe68b9986ba839a8be046ae7adb6eceaa7da5b8861e1f81131b9 --exe $exe
if ($LASTEXITCODE -ne 0) { throw 'Exact seven-size embedded PE icon verification failed.' }
$iconProof = $iconJson | ConvertFrom-Json -Depth 30
$exeHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class GridCaptureWindow {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; public Point(int x, int y) { X=x; Y=y; } }
    public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point p);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder title, int count);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int attribute, out Rect rect, int size);
    [DllImport("dwmapi.dll", EntryPoint="DwmGetWindowAttribute")] public static extern int DwmGetCloaked(IntPtr hwnd, int attribute, out int value, int size);
    [DllImport("dwmapi.dll")] public static extern int DwmFlush();
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool EnumDisplaySettings(string name, int mode, IntPtr data);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int ChangeDisplaySettings(IntPtr data, uint flags);
    public static void EnsureDesktop() {
        if (GetSystemMetrics(0) >= 1280 && GetSystemMetrics(1) >= 900) return;
        // DEVMODEW: preserve current settings except its documented width/height fields.
        IntPtr p = Marshal.AllocHGlobal(220);
        try {
            Marshal.Copy(new byte[220], 0, p, 220); Marshal.WriteInt16(p, 68, 220);
            if (!EnumDisplaySettings(null, -1, p)) throw new Exception("Read hosted display mode failed");
            Marshal.WriteInt32(p, 72, 0x180000); Marshal.WriteInt32(p, 172, 1600); Marshal.WriteInt32(p, 176, 1000);
            if (ChangeDisplaySettings(p, 0) != 0) throw new Exception("Hosted display cannot fit the required native window");
        } finally { Marshal.FreeHGlobal(p); }
    }
    public static IntPtr Find(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, d) => { uint owner; GetWindowThreadProcessId(h, out owner);
            if (owner == pid && IsWindowVisible(h)) { found=h; return false; } return true; }, IntPtr.Zero);
        return found;
    }
    public static uint Owner(IntPtr hwnd) { uint pid; GetWindowThreadProcessId(hwnd, out pid); return pid; }
    public static string Title(IntPtr hwnd) { var b=new StringBuilder(1024); GetWindowText(hwnd,b,b.Capacity); return b.ToString(); }
}
'@
if ([GridCaptureWindow]::SetThreadDpiAwarenessContext([IntPtr](-4)) -eq [IntPtr]::Zero) {
    throw 'Could not establish physical-pixel DPI awareness for the capture thread.'
}
[GridCaptureWindow]::EnsureDesktop()

function Send-Control($Writer, $Reader, [string]$Method, [hashtable]$Params = @{}) {
    $requestId = [guid]::NewGuid().ToString('N')
    $Writer.WriteLine((@{ id = $requestId; method = $Method; params = $Params } | ConvertTo-Json -Compress -Depth 20))
    $line = $Reader.ReadLine()
    if (-not $line) { throw "Control channel closed during $Method" }
    $reply = $line | ConvertFrom-Json -Depth 100
    if ($reply.id -ne $requestId -or -not $reply.ok) { throw "Control method $Method failed: $line" }
    return $reply.result
}

function Capture-Sample([string]$Sample, [string]$Stem, [string]$ExpectedTitle) {
    $profile = Join-Path $work $Sample
    New-Item -ItemType Directory -Path "$profile/roaming", "$profile/local" | Out-Null
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start(); $port = $listener.LocalEndpoint.Port; $listener.Stop()
    $start = [Diagnostics.ProcessStartInfo]::new($exe)
    $start.UseShellExecute = $false
    $start.ArgumentList.Add('--control'); $start.ArgumentList.Add([string]$port)
    $start.ArgumentList.Add('--sample'); $start.ArgumentList.Add($Sample)
    $start.WorkingDirectory = $profile
    $start.Environment['GRIDCRAFT_NO_PREFS'] = '1'
    $start.Environment['APPDATA'] = "$profile/roaming"
    $start.Environment['LOCALAPPDATA'] = "$profile/local"
    $start.Environment.Remove('GH_TOKEN') | Out-Null
    $start.Environment.Remove('GITHUB_TOKEN') | Out-Null
    $process = [Diagnostics.Process]::Start($start)
    $client = $null; $reader = $null; $writer = $null
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(45)
        while ([DateTime]::UtcNow -lt $deadline) {
            if ($process.HasExited) { throw 'Retained native process exited before startup.' }
            $attempt = [Net.Sockets.TcpClient]::new()
            try { $attempt.Connect('127.0.0.1', $port); $client = $attempt; break } catch { $attempt.Dispose(); Start-Sleep -Milliseconds 250 }
        }
        if (-not $client) { throw 'Native control startup timed out.' }
        $stream = $client.GetStream(); $stream.ReadTimeout = 15000; $stream.WriteTimeout = 15000
        $reader = [IO.StreamReader]::new($stream)
        $writer = [IO.StreamWriter]::new($stream, [Text.UTF8Encoding]::new($false)); $writer.AutoFlush = $true
        Send-Control $writer $reader 'ui.set' @{ dark = $false; animations = $false } | Out-Null
        Send-Control $writer $reader 'ui.resize' @{ width = 1120; height = 720 } | Out-Null
        Send-Control $writer $reader 'ui.ribbon' @{ tab = $(if ($Sample -eq 'sales') { 'Home' } else { 'Insert' }) } | Out-Null
        Send-Control $writer $reader 'ui.focus' | Out-Null
        $document = Send-Control $writer $reader 'document.inspect'
        $cell = Send-Control $writer $reader 'cell.get' @{ cell = 'G5' }
        if ($document.title -ne $ExpectedTitle) { throw 'The native app did not load the requested original sample.' }
        if ($Sample -eq 'sales' -and ($cell.formula -ne '=SUM(C5:F5)' -or $cell.value -ne 217420)) {
            throw 'Sales sample G5 formula/value changed.'
        }
        Start-Sleep -Milliseconds 400
        $hwnd = [GridCaptureWindow]::Find([uint32]$process.Id)
        if ($hwnd -eq [IntPtr]::Zero) { throw 'No visible window belongs to the retained process.' }
        [GridCaptureWindow]::SetWindowPos($hwnd, [IntPtr]::Zero, 40, 40, 0, 0, 0x0045) | Out-Null
        [GridCaptureWindow]::SetForegroundWindow($hwnd) | Out-Null
        Send-Control $writer $reader 'ui.focus' | Out-Null
        $clientPath = Join-Path $output "$Stem-client.png"
        $clientShot = Send-Control $writer $reader 'ui.screenshot' @{ path = $clientPath }
        [GridCaptureWindow]::DwmFlush() | Out-Null
        $owner = [GridCaptureWindow]::Owner($hwnd)
        $foreground = [GridCaptureWindow]::GetForegroundWindow()
        $cloaked = 0
        if ($owner -ne $process.Id -or $foreground -ne $hwnd -or [GridCaptureWindow]::IsIconic($hwnd) -or
            [GridCaptureWindow]::DwmGetCloaked($hwnd, 14, [ref]$cloaked, 4) -ne 0 -or $cloaked -ne 0) {
            throw 'The owned native window is not the visible foreground capture target.'
        }
        $windowRect = [GridCaptureWindow+Rect]::new()
        if (-not [GridCaptureWindow]::GetWindowRect($hwnd, [ref]$windowRect)) { throw 'GetWindowRect failed.' }
        $bounds = [GridCaptureWindow+Rect]::new()
        $method = 'DWM extended frame bounds'
        if ([GridCaptureWindow]::DwmGetWindowAttribute($hwnd, 9, [ref]$bounds, 16) -ne 0) { $bounds = $windowRect; $method = 'GetWindowRect' }
        $width = $bounds.Right - $bounds.Left; $height = $bounds.Bottom - $bounds.Top
        if ($width -lt 640 -or $height -lt 420 -or $bounds.Left -lt 0 -or $bounds.Top -lt 0 -or
            $bounds.Right -gt [GridCaptureWindow]::GetSystemMetrics(0) -or $bounds.Bottom -gt [GridCaptureWindow]::GetSystemMetrics(1)) {
            throw 'Native outer bounds are invalid or extend beyond the hosted display.'
        }
        $title = [GridCaptureWindow]::Title($hwnd)
        if ($title -notlike "*$ExpectedTitle*") { throw 'Unexpected native caption for the selected sample.' }
        foreach ($x in @(($bounds.Left + 10), ($bounds.Left + [int]($width / 2)), ($bounds.Right - 10))) {
            foreach ($y in @(($bounds.Top + 10), ($bounds.Top + [int]($height / 2)), ($bounds.Bottom - 10))) {
                $at = [GridCaptureWindow]::WindowFromPoint([GridCaptureWindow+Point]::new($x, $y))
                if ([GridCaptureWindow]::Owner($at) -ne $process.Id) { throw 'Another process obscures the app capture.' }
            }
        }
        $outerPath = Join-Path $output "$Stem.png"
        # Direct screen acquisition of the owned HWND's outer bounds: no overlays or pixel edits.
        $bitmap = [Drawing.Bitmap]::new($width, $height)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, [Drawing.Size]::new($width, $height))
            $bitmap.Save($outerPath, [Drawing.Imaging.ImageFormat]::Png)
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
        if ([GridCaptureWindow]::GetForegroundWindow() -ne $hwnd) { throw 'Foreground changed during capture.' }
        $images = @()
        foreach ($file in @($outerPath, $clientPath)) {
            $image = [Drawing.Image]::FromFile($file)
            try {
                if ($image.RawFormat.Guid -ne [Drawing.Imaging.ImageFormat]::Png.Guid -or $image.Width -lt 640 -or $image.Height -lt 420) { throw 'Invalid native PNG capture.' }
                $images += @{ name = [IO.Path]::GetFileName($file); width = $image.Width; height = $image.Height; sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() }
            } finally { $image.Dispose() }
        }
        Send-Control $writer $reader 'app.quit' | Out-Null
        if (-not $process.WaitForExit(10000)) { throw 'The native app did not exit after app.quit.' }
        if ($process.ExitCode -ne 0) { throw 'The native app exited unsuccessfully.' }
        return @{ sample = $Sample; title = $title; processId = $process.Id; hwnd = $hwnd.ToInt64(); foregroundOwned = $true; outerBoundsMethod = $method; outerBounds = @($bounds.Left, $bounds.Top, $bounds.Right, $bounds.Bottom); getWindowRect = @($windowRect.Left, $windowRect.Top, $windowRect.Right, $windowRect.Bottom); document = $document; cellG5 = $cell; clientScreenshot = $clientShot; images = $images }
    } finally {
        if ($writer) { $writer.Dispose() }; if ($reader) { $reader.Dispose() }; if ($client) { $client.Dispose() }
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
}

$captures = @(
    Capture-Sample 'sales' 'gridstrike-sales-dashboard' 'Quarterly Sales Dashboard'
    Capture-Sample 'budget' 'gridstrike-budget-analysis' 'Household Budget'
)
$proof = [ordered]@{
    source = 'Direct hosted Windows native app captures; no overlays, mockups or pixel edits'
    buildRun = $buildRun; buildHead = $buildHead; buildTree = $baseTree; currentMain = $main.commit.sha
    artifactId = $artifactId; artifactZipSha256 = $zipHash; executableSha256 = $exeHash
    captureRun = $env:GITHUB_RUN_ID; captureCommit = $env:GITHUB_SHA
    preferencesDisabled = $true; inputs = 'Original sales and budget samples; isolated temporary APPDATA and LOCALAPPDATA'
    icon = $iconProof; captures = $captures
    limits = 'Hosted appearance only; Room, other DPI settings, broad workbook compatibility and release readiness are not verified.'
}
$proof | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath (Join-Path $output 'proof.json') -Encoding utf8NoBOM
Write-Host "Captured both original samples from the exact retained executable ($exeHash)."
