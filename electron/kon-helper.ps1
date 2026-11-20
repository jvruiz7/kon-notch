# Kon helper — processo PowerShell persistente que lê mídia (SMTC) e volume (Core Audio) do Windows.
# Saída: uma linha JSON por evento em stdout. Entrada: comandos JSON por linha em stdin.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

function Emit($obj) {
  try { [Console]::Out.WriteLine(($obj | ConvertTo-Json -Compress -Depth 4)); [Console]::Out.Flush() } catch {}
}

# ───────── Volume mestre (Core Audio via COM) ─────────
$volOk = $false
try {
  Add-Type -Language CSharp -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

[Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IKonAudioEndpointVolume {
  int f(); int g(); int h(); int i();
  int SetMasterVolumeLevelScalar(float fLevel, Guid pguidEventContext);
  int j();
  int GetMasterVolumeLevelScalar(out float pfLevel);
  int k(); int l(); int m(); int n();
  int SetMute([MarshalAs(UnmanagedType.Bool)] bool bMute, Guid pguidEventContext);
  int GetMute(out bool pbMute);
}
[Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IKonMMDevice {
  int Activate(ref Guid id, int clsCtx, int activationParams, out IKonAudioEndpointVolume aev);
}
[Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IKonMMDeviceEnumerator {
  int f();
  int GetDefaultAudioEndpoint(int dataFlow, int role, out IKonMMDevice endpoint);
}
[ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class KonMMDeviceEnumerator { }

public class KonAudio {
  static IKonAudioEndpointVolume cached;
  static IKonAudioEndpointVolume Vol() {
    if (cached != null) return cached;
    var enumerator = new KonMMDeviceEnumerator() as IKonMMDeviceEnumerator;
    IKonMMDevice dev = null;
    Marshal.ThrowExceptionForHR(enumerator.GetDefaultAudioEndpoint(0, 1, out dev));
    IKonAudioEndpointVolume epv = null;
    var id = typeof(IKonAudioEndpointVolume).GUID;
    Marshal.ThrowExceptionForHR(dev.Activate(ref id, 23, 0, out epv));
    cached = epv;
    return epv;
  }
  public static void Reset() { cached = null; }
  public static float Get() { float v = -1; Marshal.ThrowExceptionForHR(Vol().GetMasterVolumeLevelScalar(out v)); return v; }
  public static bool Muted() { bool m = false; Marshal.ThrowExceptionForHR(Vol().GetMute(out m)); return m; }
}
"@
  $volOk = $true
} catch { Emit @{ type = 'error'; scope = 'volume'; message = $_.Exception.Message } }

# ───────── Janela em primeiro plano (tela cheia / maximizada) ─────────
$fgOk = $false
try {
  Add-Type -Language CSharp -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class KonFg {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct MONITORINFO { public int cbSize; public RECT rcMonitor; public RECT rcWork; public int dwFlags; }
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr h, uint f);
  [DllImport("user32.dll")] static extern bool GetMonitorInfo(IntPtr m, ref MONITORINFO mi);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("shell32.dll")] static extern int SHQueryUserNotificationState(out int s);
  public static string State() {
    IntPtr h = GetForegroundWindow();
    if (h == IntPtr.Zero || !IsWindowVisible(h)) return "0|0|0|0|0";
    var cls = new StringBuilder(256); GetClassName(h, cls, 256);
    string c = cls.ToString();
    uint pid; GetWindowThreadProcessId(h, out pid);
    var mi = new MONITORINFO(); mi.cbSize = Marshal.SizeOf(typeof(MONITORINFO));
    GetMonitorInfo(MonitorFromWindow(h, 2), ref mi);
    RECT r; GetWindowRect(h, out r);
    bool shell = c == "Progman" || c == "WorkerW" || c == "Shell_TrayWnd" || c == "Shell_SecondaryTrayWnd";
    bool covers = r.L <= mi.rcMonitor.L && r.T <= mi.rcMonitor.T && r.R >= mi.rcMonitor.R && r.B >= mi.rcMonitor.B;
    int q = 0; try { SHQueryUserNotificationState(out q); } catch { }
    // janela maximizada passa ~8 px da borda do monitor: não é tela cheia
    bool zoomed = IsZoomed(h);
    bool full = !shell && ((covers && !zoomed) || q == 3 || q == 4);
    bool max = !shell && !full && zoomed;
    int cx = (mi.rcMonitor.L + mi.rcMonitor.R) / 2, cy = (mi.rcMonitor.T + mi.rcMonitor.B) / 2;
    return (full ? "1" : "0") + "|" + (max ? "1" : "0") + "|" + cx + "|" + cy + "|" + pid;
  }
}
"@
  $fgOk = $true
} catch { Emit @{ type = 'error'; scope = 'fg'; message = $_.Exception.Message } }

# ───────── Microfone / câmera em uso (ConsentStore do Windows) ─────────
function Get-InUse($cap) {
  $base = "HKCU:\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\$cap"
  $names = New-Object System.Collections.Generic.List[string]
  $keys = @()
  $keys += @(Get-ChildItem -LiteralPath $base -ErrorAction SilentlyContinue)
  $keys += @(Get-ChildItem -LiteralPath "$base\NonPackaged" -ErrorAction SilentlyContinue)
  foreach ($k in $keys) {
    if ($null -eq $k -or $k.PSChildName -eq 'NonPackaged') { continue }
    try {
      $start = $k.GetValue('LastUsedTimeStart'); $stop = $k.GetValue('LastUsedTimeStop')
      if ($null -ne $start -and [long]$start -gt 0 -and $null -ne $stop -and [long]$stop -eq 0) {
        $n = $k.PSChildName
        if ($n -like '*#*') { $n = (($n -split '#')[-1]) -replace '\.exe$', '' } else { $n = ($n -split '_')[0]; $n = ($n -split '\.')[-1] }
        if (-not $names.Contains($n)) { $names.Add($n) }
      }
    } catch {}
  }
  return ,$names.ToArray()
}
$script:lastFg = ''
$script:lastPriv = ''

# ───────── Mídia (Global System Media Transport Controls) ─────────
$mediaOk = $false
$mgr = $null
function Await($op, [Type]$type, [int]$ms = 8000) {
  $task = $script:asTaskGeneric.MakeGenericMethod($type).Invoke($null, @($op))
  if ($task.Wait($ms)) { return $task.Result }
  return $null
}
try {
  Add-Type -AssemblyName System.Runtime.WindowsRuntime
  $script:asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
      $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
    })[0]
} catch { Emit @{ type = 'error'; scope = 'winrt'; message = $_.Exception.Message } }
try {
  $null = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager, Windows.Media.Control, ContentType=WindowsRuntime]
  $null = [Windows.Storage.Streams.DataReader, Windows.Storage.Streams, ContentType=WindowsRuntime]
  $null = [Windows.Media.MediaPlaybackAutoRepeatMode, Windows.Media, ContentType=WindowsRuntime]
  $mgr = Await ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager]::RequestAsync()) ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager])
  $mediaOk = $null -ne $mgr
} catch { Emit @{ type = 'error'; scope = 'media'; message = $_.Exception.Message } }

function Get-Thumb($ref) {
  if ($null -eq $ref) { return $null }
  try {
    $stream = Await ($ref.OpenReadAsync()) ([Windows.Storage.Streams.IRandomAccessStreamWithContentType]) 1500
    if ($null -eq $stream) { return $null }
    $mime = $stream.ContentType
    # Converte o stream WinRT em stream .NET (bem mais confiável que DataReader no PowerShell)
    $net = [System.IO.WindowsRuntimeStreamExtensions]::AsStreamForRead($stream)
    $ms = New-Object System.IO.MemoryStream
    $net.CopyTo($ms)
    $bytes = $ms.ToArray()
    $ms.Dispose(); $net.Dispose(); $stream.Dispose()
    if ($bytes.Length -lt 64 -or $bytes.Length -gt 6MB) { return $null }
    if (-not $mime -or $mime -notlike 'image/*') {
      if ($bytes[0] -eq 0xFF -and $bytes[1] -eq 0xD8) { $mime = 'image/jpeg' } else { $mime = 'image/png' }
    }
    $hash = [BitConverter]::ToString([System.Security.Cryptography.MD5]::Create().ComputeHash($bytes))
    return @{ mime = $mime; data = [Convert]::ToBase64String($bytes); hash = $hash }
  } catch { return $null }
}

$script:lastKey = ''
$script:thumbTries = 0
$script:thumbDone = $false
$script:prevThumbHash = ''
$script:curThumbHash = ''
$script:lastSig = ''
$script:lastEmitAt = [DateTime]::MinValue
function Emit-Media([bool]$force = $false) {
  if (-not $mediaOk) { return }
  try {
    $s = $mgr.GetCurrentSession()
    if ($null -eq $s) {
      if ($script:lastKey -ne 'none') { $script:lastKey = 'none'; Emit @{ type = 'media'; active = $false } }
      return
    }
    $p = Await ($s.TryGetMediaPropertiesAsync()) ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties]) 2000
    if ($null -eq $p) { return }
    $info = $s.GetPlaybackInfo()
    $key = "$($s.SourceAppUserModelId)|$($p.Title)|$($p.Artist)"
    $changed = $false
    if ($key -ne $script:lastKey) {
      $script:lastKey = $key; $changed = $true; $script:thumbTries = 0; $script:thumbDone = $false
      $script:prevThumbHash = $script:curThumbHash; $script:curThumbHash = ''
    }
    # A capa costuma chegar alguns instantes depois da troca de faixa: tenta a cada 250 ms nos
    # primeiros 4 s, depois a cada 1 s até 15 s. Ignora a capa da faixa anterior (Spotify às vezes repete).
    $thumb = $null
    if (-not $script:thumbDone -and $script:thumbTries -lt 27) {
      $fast = $script:thumbTries -lt 16
      if ($fast -or $script:tick % 4 -eq 0) {
        $script:thumbTries++
        $t = Get-Thumb $p.Thumbnail
        if ($t -and ($t.hash -ne $script:prevThumbHash -or $script:thumbTries -ge 6)) {
          $thumb = $t; $script:thumbDone = $true; $script:curThumbHash = $t.hash
        }
      }
    }
    $status = [int]$info.PlaybackStatus
    $repeat = 0; if ($null -ne $info.AutoRepeatMode) { $repeat = [int]$info.AutoRepeatMode }
    $shuffle = $false; if ($null -ne $info.IsShuffleActive) { $shuffle = [bool]$info.IsShuffleActive }
    $sig = "$key|$status|$repeat|$shuffle"
    $now = [DateTime]::UtcNow
    # só manda quando algo mudou, quando achou a capa, ou 1x por segundo (posição)
    if (-not $force -and -not $changed -and -not $thumb -and $sig -eq $script:lastSig -and ($now - $script:lastEmitAt).TotalMilliseconds -lt 950) { return }
    $script:lastSig = $sig; $script:lastEmitAt = $now
    $tl = $s.GetTimelineProperties()
    if ($thumb) { $thumb.Remove('hash') }
    Emit @{
      type = 'media'; active = $true; app = $s.SourceAppUserModelId
      title = $p.Title; artist = $p.Artist; album = $p.AlbumTitle
      status = $status
      position = $tl.Position.TotalSeconds; duration = ($tl.EndTime - $tl.StartTime).TotalSeconds
      updated = $tl.LastUpdatedTime.ToUnixTimeMilliseconds()
      shuffle = $shuffle; repeat = $repeat
      canSeek = [bool]$info.Controls.IsPlaybackPositionEnabled
      trackChanged = $changed; thumb = $thumb
    }
  } catch { Emit @{ type = 'error'; scope = 'media'; message = $_.Exception.Message } }
}

# ───────── OCR nativo (Windows.Media.Ocr) ─────────
$script:ocrTypes = $false
function Invoke-Ocr($c) {
  try {
    if (-not $script:ocrTypes) {
      Add-Type -AssemblyName System.Runtime.WindowsRuntime
      $null = [Windows.Storage.StorageFile, Windows.Storage, ContentType=WindowsRuntime]
      $null = [Windows.Storage.FileAccessMode, Windows.Storage, ContentType=WindowsRuntime]
      $null = [Windows.Storage.Streams.IRandomAccessStream, Windows.Storage.Streams, ContentType=WindowsRuntime]
      $null = [Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics, ContentType=WindowsRuntime]
      $null = [Windows.Graphics.Imaging.SoftwareBitmap, Windows.Foundation, ContentType=WindowsRuntime]
      $null = [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType=WindowsRuntime]
      $null = [Windows.Media.Ocr.OcrResult, Windows.Foundation, ContentType=WindowsRuntime]
      $null = [Windows.Globalization.Language, Windows.Globalization, ContentType=WindowsRuntime]
      $script:ocrTypes = $true
    }
    $file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync([string]$c.path)) ([Windows.Storage.StorageFile])
    $stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
    $decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
    $bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
    $engine = $null
    foreach ($tag in @([string]$c.lang, 'pt-BR', 'en-US')) {
      if ($engine) { break }
      try { if ($tag) { $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage((New-Object Windows.Globalization.Language($tag))) } } catch {}
    }
    if (-not $engine) { $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages() }
    if (-not $engine) { throw 'Nenhum idioma de OCR instalado no Windows (Configurações > Idioma > adicionar Português (Brasil))' }
    $result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
    $lines = @()
    if ($result) { foreach ($l in $result.Lines) { $lines += $l.Text } }
    try { $stream.Dispose() } catch {}
    Emit @{ type = 'ocr'; id = $c.id; text = ($lines -join "`n"); lang = $engine.RecognizerLanguage.LanguageTag }
  } catch { Emit @{ type = 'ocr'; id = $c.id; error = $_.Exception.Message } }
}

function Invoke-Cmd($c) {
  if ($c.cmd -eq 'ocr') { Invoke-Ocr $c; return }
  if (-not $mediaOk) { return }
  $s = $mgr.GetCurrentSession()
  if ($null -eq $s) { return }
  switch ($c.cmd) {
    'toggle'  { $null = Await ($s.TryTogglePlayPauseAsync()) ([bool]) }
    'play'    { $null = Await ($s.TryPlayAsync()) ([bool]) }
    'pause'   { $null = Await ($s.TryPauseAsync()) ([bool]) }
    'next'    { $null = Await ($s.TrySkipNextAsync()) ([bool]) }
    'prev'    { $null = Await ($s.TrySkipPreviousAsync()) ([bool]) }
    'seek'    { $null = Await ($s.TryChangePlaybackPositionAsync([long]([double]$c.value * 10000000))) ([bool]) }
    'shuffle' { $null = Await ($s.TryChangeShuffleActiveAsync([bool]$c.value)) ([bool]) }
    'repeat'  { $null = Await ($s.TryChangeAutoRepeatModeAsync([Windows.Media.MediaPlaybackAutoRepeatMode][int]$c.value)) ([bool]) }
  }
  Start-Sleep -Milliseconds 120
  Emit-Media $true
}

Emit @{ type = 'ready'; volume = $volOk; media = $mediaOk }

# ───────── Loop principal ─────────
$stdin = New-Object System.IO.StreamReader([Console]::OpenStandardInput())
$pending = $stdin.ReadLineAsync()
$lastVol = ''
$script:tick = 0
while ($true) {
  # processa todos os comandos que chegaram (antes era 1 por volta de 250 ms e os cliques formavam fila)
  $closed = $false
  $batch = New-Object System.Collections.Generic.List[object]
  while ($pending.IsCompleted) {
    $line = $null
    try { $line = $pending.Result } catch { $closed = $true; break }
    if ($null -eq $line) { $closed = $true; break } # o processo pai fechou o stdin
    if ($line.Trim()) { try { $batch.Add(($line | ConvertFrom-Json)) } catch {} }
    $pending = $stdin.ReadLineAsync()
  }
  if ($closed) { break }
  # cliques repetidos de play/pause se anulam: só o resultado final importa
  $toggles = @($batch | Where-Object { $_.cmd -eq 'toggle' }).Count
  $didToggle = $false
  foreach ($c in $batch) {
    if ($c.cmd -eq 'toggle') { if ($didToggle -or ($toggles % 2 -eq 0)) { continue }; $didToggle = $true }
    try { Invoke-Cmd $c } catch { Emit @{ type = 'error'; scope = 'cmd'; message = $_.Exception.Message } }
  }
  if ($volOk) {
    try {
      $v = [math]::Round([KonAudio]::Get() * 100)
      $m = [KonAudio]::Muted()
      $k = "$v|$m"
      if ($k -ne $lastVol) { $lastVol = $k; Emit @{ type = 'volume'; level = $v; muted = $m } }
    } catch { [KonAudio]::Reset() }
  }
  Emit-Media
  if ($fgOk -and $script:tick % 2 -eq 0) {
    try {
      $st = [KonFg]::State()
      if ($st -ne $script:lastFg) {
        $script:lastFg = $st
        $p = $st -split '\|'
        Emit @{ type = 'fg'; full = ($p[0] -eq '1'); max = ($p[1] -eq '1'); mx = [int]$p[2]; my = [int]$p[3]; pid = [int]$p[4] }
      }
    } catch {}
  }
  if ($script:tick % 8 -eq 2) {
    try {
      $mic = Get-InUse 'microphone'; $cam = Get-InUse 'webcam'
      $k = ($mic -join ',') + '|' + ($cam -join ',')
      if ($k -ne $script:lastPriv) { $script:lastPriv = $k; Emit @{ type = 'privacy'; mic = @($mic); cam = @($cam) } }
    } catch {}
  }
  $script:tick++
  Start-Sleep -Milliseconds 250
}
