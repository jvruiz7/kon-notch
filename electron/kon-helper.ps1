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

# ───────── Saídas de áudio e volume por app (Core Audio) ─────────
$script:mixOk = $null
function Ensure-Mix {
  if ($null -ne $script:mixOk) { return $script:mixOk }
  try {
  Add-Type -Language CSharp -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

[StructLayout(LayoutKind.Sequential)] public struct KAPropKey { public Guid fmtid; public int pid; }
[StructLayout(LayoutKind.Explicit, Size = 24)] public struct KAPropVar { [FieldOffset(0)] public short vt; [FieldOffset(8)] public IntPtr p; }

[Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIPropertyStore {
  int GetCount(out int c);
  int GetAt(int i, out KAPropKey k);
  int GetValue(ref KAPropKey k, out KAPropVar v);
}
[Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIMMDevice {
  int Activate(ref Guid iid, int ctx, IntPtr p, [MarshalAs(UnmanagedType.IUnknown)] out object o);
  int OpenPropertyStore(int access, out KAIPropertyStore store);
  int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
  int GetState(out int state);
}
[Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIMMDeviceCollection {
  int GetCount(out int c);
  int Item(int i, out KAIMMDevice d);
}
[Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIMMDeviceEnumerator {
  int EnumAudioEndpoints(int flow, int mask, out KAIMMDeviceCollection col);
  int GetDefaultAudioEndpoint(int flow, int role, out KAIMMDevice d);
}
[ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class KAEnumerator { }

[Guid("F8679F50-850A-41CF-9C72-430F290290C8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIPolicyConfig {
  int a1(); int a2(); int a3(); int a4(); int a5(); int a6(); int a7(); int a8(); int a9(); int a10();
  int SetDefaultEndpoint([MarshalAs(UnmanagedType.LPWStr)] string id, int role);
}
[ComImport, Guid("870af99c-171d-4f9e-af0d-e63df40c5bc9")] class KAPolicyConfigClient { }

[Guid("87CE5498-68D6-44E5-9215-6DA47EF883D8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAISimpleAudioVolume {
  int SetMasterVolume(float v, ref Guid ctx);
  int GetMasterVolume(out float v);
  int SetMute([MarshalAs(UnmanagedType.Bool)] bool m, ref Guid ctx);
  int GetMute([MarshalAs(UnmanagedType.Bool)] out bool m);
}
[Guid("bfb7ff88-7239-4fc9-8fa2-07c950be9c6d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIAudioSessionControl2 {
  int GetState(out int s);
  int GetDisplayName([MarshalAs(UnmanagedType.LPWStr)] out string n);
  int SetDisplayName([MarshalAs(UnmanagedType.LPWStr)] string n, ref Guid ctx);
  int GetIconPath([MarshalAs(UnmanagedType.LPWStr)] out string p);
  int SetIconPath([MarshalAs(UnmanagedType.LPWStr)] string p, ref Guid ctx);
  int GetGroupingParam(out Guid g);
  int SetGroupingParam(ref Guid g, ref Guid ctx);
  int RegisterAudioSessionNotification(IntPtr n);
  int UnregisterAudioSessionNotification(IntPtr n);
  int GetSessionIdentifier([MarshalAs(UnmanagedType.LPWStr)] out string id);
  int GetSessionInstanceIdentifier([MarshalAs(UnmanagedType.LPWStr)] out string id);
  int GetProcessId(out uint pid);
  [PreserveSig] int IsSystemSoundsSession();
}
[Guid("E2F5BB11-0570-40CA-ACDD-3AA01277DEE8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIAudioSessionEnumerator {
  int GetCount(out int c);
  int GetSession(int i, out KAIAudioSessionControl2 s);
}
[Guid("77AA99A0-1BD6-484F-8BC7-2C654C9A9B6F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface KAIAudioSessionManager2 {
  int GetAudioSessionControl(IntPtr g, int f, out IntPtr c);
  int GetSimpleAudioVolume(IntPtr g, int f, out IntPtr v);
  int GetSessionEnumerator(out KAIAudioSessionEnumerator e);
}

public class KonMix {
  static KAIMMDeviceEnumerator En() { return (KAIMMDeviceEnumerator)(new KAEnumerator()); }
  static string Name(KAIMMDevice d) {
    try {
      KAIPropertyStore st; d.OpenPropertyStore(0, out st);
      var k = new KAPropKey { fmtid = new Guid("a45c254e-df1c-4efd-8020-67d146a850e0"), pid = 14 };
      KAPropVar v; st.GetValue(ref k, out v);
      if (v.vt == 31 && v.p != IntPtr.Zero) return Marshal.PtrToStringUni(v.p);
    } catch { }
    return "Dispositivo";
  }
  public static string DefaultId() {
    KAIMMDevice d; En().GetDefaultAudioEndpoint(0, 1, out d);
    string id; d.GetId(out id); return id;
  }
  // "id\tnome" por linha, só saídas ativas
  public static string[] Devices() {
    var list = new List<string>();
    KAIMMDeviceCollection col; En().EnumAudioEndpoints(0, 1, out col);
    int n; col.GetCount(out n);
    for (int i = 0; i < n; i++) { KAIMMDevice d; col.Item(i, out d); string id; d.GetId(out id); list.Add(id + "\t" + Name(d)); }
    return list.ToArray();
  }
  public static void SetDefault(string id) {
    var pc = (KAIPolicyConfig)(new KAPolicyConfigClient());
    for (int role = 0; role < 3; role++) pc.SetDefaultEndpoint(id, role);
  }
  static List<KeyValuePair<uint, KAISimpleAudioVolume>> Sessions() {
    var res = new List<KeyValuePair<uint, KAISimpleAudioVolume>>();
    KAIMMDevice d; En().GetDefaultAudioEndpoint(0, 1, out d);
    Guid iid = typeof(KAIAudioSessionManager2).GUID; object o;
    d.Activate(ref iid, 23, IntPtr.Zero, out o);
    var mgr = (KAIAudioSessionManager2)o;
    KAIAudioSessionEnumerator e; mgr.GetSessionEnumerator(out e);
    int n; e.GetCount(out n);
    for (int i = 0; i < n; i++) {
      try {
        KAIAudioSessionControl2 c; e.GetSession(i, out c);
        int state; c.GetState(out state);
        if (state == 2) continue; // expirada
        uint pid; c.GetProcessId(out pid);
        res.Add(new KeyValuePair<uint, KAISimpleAudioVolume>(pid, (KAISimpleAudioVolume)c));
      } catch { }
    }
    return res;
  }
  // "pid\tvolume\tmudo" por sessão
  public static string[] Apps() {
    var list = new List<string>();
    foreach (var s in Sessions()) {
      try { float v; bool m; s.Value.GetMasterVolume(out v); s.Value.GetMute(out m); list.Add(s.Key + "\t" + Math.Round(v * 100) + "\t" + (m ? 1 : 0)); } catch { }
    }
    return list.ToArray();
  }
  public static void SetApp(uint[] pids, int vol, int mute) {
    var g = Guid.Empty;
    foreach (var s in Sessions()) {
      if (Array.IndexOf(pids, s.Key) < 0) continue;
      try { if (vol >= 0) s.Value.SetMasterVolume(Math.Max(0, Math.Min(100, vol)) / 100f, ref g); if (mute >= 0) s.Value.SetMute(mute == 1, ref g); } catch { }
    }
  }
}
"@
    $script:mixOk = $true
  } catch { $script:mixOk = $false; Emit @{ type = 'error'; scope = 'mix'; message = $_.Exception.Message } }
  return $script:mixOk
}

function Invoke-Mix($c) {
  if (-not (Ensure-Mix)) { Emit @{ type = 'mix'; id = $c.id; ok = $false }; return }
  try {
    if ($c.cmd -eq 'setdev') { [KonMix]::SetDefault([string]$c.value) }
    elseif ($c.cmd -eq 'appvol') { [KonMix]::SetApp([uint32[]]@($c.pids), [int]$c.volume, -1) }
    elseif ($c.cmd -eq 'appmute') { [KonMix]::SetApp([uint32[]]@($c.pids), -1, [int]$c.mute) }
    $def = ''; try { $def = [KonMix]::DefaultId() } catch {}
    $devs = @(); foreach ($l in [KonMix]::Devices()) { $p = $l -split "`t", 2; $devs += @{ id = $p[0]; name = $p[1]; default = ($p[0] -eq $def) } }
    $apps = @{}
    foreach ($l in [KonMix]::Apps()) {
      $p = $l -split "`t"; $procId = [int]$p[0]
      $name = 'Sons do sistema'; $title = ''
      if ($procId -ne 0) { try { $pr = Get-Process -Id $procId -ErrorAction Stop; $name = $pr.ProcessName; try { $title = $pr.MainModule.FileVersionInfo.FileDescription } catch {} } catch { continue } }
      if (-not $title) { $title = $name }
      if ($apps.ContainsKey($name)) { $apps[$name].pids += $procId }
      else { $apps[$name] = @{ name = $name; title = $title; pids = @($procId); volume = [int]$p[1]; muted = ($p[2] -eq '1') } }
    }
    Emit @{ type = 'mix'; id = $c.id; ok = $true; devices = @($devs); apps = @($apps.Values) }
  } catch { Emit @{ type = 'mix'; id = $c.id; ok = $false; error = $_.Exception.Message } }
}

# ───────── Não incomodar do Windows (Assistente de foco, via WNF) e processos ─────────
$script:dndOk = $null
function Ensure-Dnd {
  if ($null -ne $script:dndOk) { return $script:dndOk }
  try {
  Add-Type -Language CSharp -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class KonDnd {
  [DllImport("ntdll.dll")] static extern int ZwUpdateWnfStateData(ref ulong name, byte[] buf, int len, IntPtr type, IntPtr scope, uint match, uint check);
  [DllImport("ntdll.dll")] static extern int ZwQueryWnfStateData(ref ulong name, IntPtr type, IntPtr scope, out uint stamp, byte[] buf, ref int size);
  const ulong QUIET = 0x0D83063EA3BF1C75; // WNF_SHEL_QUIETHOURS_ACTIVE_PROFILE_CHANGED
  public static int Get() {
    ulong n = QUIET; uint stamp; var b = new byte[4]; int size = 4;
    int r = ZwQueryWnfStateData(ref n, IntPtr.Zero, IntPtr.Zero, out stamp, b, ref size);
    return r == 0 ? BitConverter.ToInt32(b, 0) : -1;
  }
  // 0 = desligado, 1 = só prioridade, 2 = só alarmes
  public static bool Set(int mode) {
    ulong n = QUIET; var b = BitConverter.GetBytes(mode);
    return ZwUpdateWnfStateData(ref n, b, 4, IntPtr.Zero, IntPtr.Zero, 0, 0) == 0;
  }
}
"@
    $script:dndOk = $true
  } catch { $script:dndOk = $false; Emit @{ type = 'error'; scope = 'dnd'; message = $_.Exception.Message } }
  return $script:dndOk
}

$script:procPrev = @{}
$script:procAt = [DateTime]::UtcNow
function Invoke-Procs($c) {
  try {
    $now = [DateTime]::UtcNow
    $secs = [Math]::Max(0.5, ($now - $script:procAt).TotalSeconds)
    $cores = [Environment]::ProcessorCount
    $agg = @{}
    foreach ($p in Get-Process -ErrorAction SilentlyContinue) {
      if ($p.Id -eq 0 -or $p.Id -eq 4) { continue }
      $cpu = 0; try { $cpu = $p.TotalProcessorTime.TotalSeconds } catch { continue }
      $prev = $script:procPrev[$p.Id]
      $delta = 0; if ($null -ne $prev) { $delta = [Math]::Max(0, $cpu - $prev) }
      $script:procPrev[$p.Id] = $cpu
      $n = $p.ProcessName
      if (-not $agg.ContainsKey($n)) { $agg[$n] = @{ name = $n; cpu = 0.0; mem = 0 } }
      $agg[$n].cpu += $delta; $agg[$n].mem += $p.WorkingSet64
    }
    $script:procAt = $now
    $top = @($agg.Values | ForEach-Object { @{ name = $_.name; cpu = [Math]::Round(100 * $_.cpu / $secs / $cores, 1); mem = [Math]::Round($_.mem / 1MB) } } | Sort-Object { -$_.cpu } | Select-Object -First 4)
    $topMem = @($agg.Values | Sort-Object { -$_.mem } | Select-Object -First 1 | ForEach-Object { @{ name = $_.name; mem = [Math]::Round($_.mem / 1MB) } })
    Emit @{ type = 'procs'; id = $c.id; top = $top; topMem = $topMem }
  } catch { Emit @{ type = 'procs'; id = $c.id; top = @(); error = $_.Exception.Message } }
}

# ───────── Palavra de ativação "Kon" (reconhecimento de voz do Windows, offline) ─────────
$script:wakeOk = $null
function Ensure-Wake {
  if ($null -ne $script:wakeOk) { return $script:wakeOk }
  try {
  Add-Type -ReferencedAssemblies System.Speech -Language CSharp -TypeDefinition @"
using System;
using System.Globalization;
using System.Speech.Recognition;
public class KonWake {
  static SpeechRecognitionEngine eng;
  static int hits;
  static float minConf = 0.6f;
  public static string Info() {
    string all = "";
    foreach (var r in SpeechRecognitionEngine.InstalledRecognizers()) all += (all.Length > 0 ? "," : "") + r.Culture.Name;
    return all;
  }
  public static string Start(float conf) {
    Stop();
    minConf = conf;
    RecognizerInfo pick = null;
    foreach (var r in SpeechRecognitionEngine.InstalledRecognizers()) {
      if (r.Culture.Name == "pt-BR") { pick = r; break; }
      if (pick == null || r.Culture.Name == "en-US") pick = r;
    }
    if (pick == null) return "sem-reconhecedor";
    eng = new SpeechRecognitionEngine(pick);
    var gb = new GrammarBuilder(new Choices(new string[] { "Kon", "Hey Kon", "Ok Kon", "Oi Kon" }));
    gb.Culture = pick.Culture;
    eng.LoadGrammar(new Grammar(gb));
    // um pouco de "lixo" para não confundir qualquer som com a palavra
    var junk = new GrammarBuilder(new Choices(new string[] { "on", "come", "can", "cone", "done", "gone", "John", "corn", "con" }));
    junk.Culture = pick.Culture;
    var jg = new Grammar(junk); jg.Name = "junk"; eng.LoadGrammar(jg);
    eng.SpeechRecognized += (s, e) => { if (e.Result.Grammar.Name != "junk" && e.Result.Confidence >= minConf) hits++; };
    eng.SetInputToDefaultAudioDevice();
    eng.RecognizeAsync(RecognizeMode.Multiple);
    return "ok:" + pick.Culture.Name;
  }
  public static void Stop() { try { if (eng != null) { eng.RecognizeAsyncCancel(); eng.Dispose(); } } catch { } eng = null; hits = 0; }
  public static int Poll() { int h = hits; hits = 0; return h; }
}
"@
    $script:wakeOk = $true
  } catch { $script:wakeOk = $false; Emit @{ type = 'error'; scope = 'wake'; message = $_.Exception.Message } }
  return $script:wakeOk
}
$script:wakeOn = $false

# ───────── Teclado: copiar/colar no app em primeiro plano (ações no texto selecionado) ─────────
$script:keysOk = $null
function Ensure-Keys {
  if ($null -ne $script:keysOk) { return $script:keysOk }
  try {
  Add-Type -Language CSharp -TypeDefinition @"
using System;
using System.Threading;
using System.Runtime.InteropServices;
public class KonKeys {
  [DllImport("user32.dll")] static extern short GetAsyncKeyState(int vk);
  [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  const uint UP = 2;
  static bool Down(int vk) { return (GetAsyncKeyState(vk) & 0x8000) != 0; }
  // espera você soltar Ctrl/Alt/Shift/Win do atalho, senão o Ctrl+C vira Ctrl+Alt+C
  public static void WaitModifiers(int ms) {
    int waited = 0;
    while (waited < ms && (Down(0x10) || Down(0x11) || Down(0x12) || Down(0x5B) || Down(0x5C))) { Thread.Sleep(15); waited += 15; }
    foreach (byte k in new byte[] { 0x12, 0x10, 0x5B }) if (Down(k)) keybd_event(k, 0, UP, UIntPtr.Zero);
  }
  public static void Ctrl(byte vk) {
    keybd_event(0x11, 0, 0, UIntPtr.Zero);
    keybd_event(vk, 0, 0, UIntPtr.Zero);
    Thread.Sleep(20);
    keybd_event(vk, 0, UP, UIntPtr.Zero);
    keybd_event(0x11, 0, UP, UIntPtr.Zero);
  }
}
"@
    $script:keysOk = $true
  } catch { $script:keysOk = $false; Emit @{ type = 'error'; scope = 'keys'; message = $_.Exception.Message } }
  return $script:keysOk
}

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
  if (@('mix', 'setdev', 'appvol', 'appmute') -contains $c.cmd) { Invoke-Mix $c; return }
  if ($c.cmd -eq 'procs') { Invoke-Procs $c; return }
  if ($c.cmd -eq 'wake') {
    if (-not (Ensure-Wake)) { Emit @{ type = 'wakeinfo'; id = $c.id; ok = $false; error = 'O reconhecimento de voz do Windows não está disponível' }; return }
    try {
      if ($c.on) { $r = [KonWake]::Start([float]$c.conf); $script:wakeOn = $r.StartsWith('ok'); Emit @{ type = 'wakeinfo'; id = $c.id; ok = $script:wakeOn; result = $r; recognizers = [KonWake]::Info() } }
      else { [KonWake]::Stop(); $script:wakeOn = $false; Emit @{ type = 'wakeinfo'; id = $c.id; ok = $true } }
    } catch { $script:wakeOn = $false; Emit @{ type = 'wakeinfo'; id = $c.id; ok = $false; error = $_.Exception.Message } }
    return
  }
  if ($c.cmd -eq 'dnd') {
    if (-not (Ensure-Dnd)) { Emit @{ type = 'dnd'; id = $c.id; ok = $false }; return }
    try {
      $prev = [KonDnd]::Get(); $ok = $true
      if ($null -ne $c.mode) { $ok = [KonDnd]::Set([int]$c.mode) }
      Emit @{ type = 'dnd'; id = $c.id; ok = $ok; prev = $prev; now = [KonDnd]::Get() }
    } catch { Emit @{ type = 'dnd'; id = $c.id; ok = $false; error = $_.Exception.Message } }
    return
  }
  if ($c.cmd -eq 'copysel' -or $c.cmd -eq 'paste') {
    if (-not (Ensure-Keys)) { Emit @{ type = 'keys'; id = $c.id; ok = $false }; return }
    try {
      [KonKeys]::WaitModifiers(1200)
      if ($c.cmd -eq 'copysel') { [KonKeys]::Ctrl(0x43) } else { [KonKeys]::Ctrl(0x56) }
      Emit @{ type = 'keys'; id = $c.id; ok = $true }
    } catch { Emit @{ type = 'keys'; id = $c.id; ok = $false } }
    return
  }
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
  if ($script:wakeOn) { try { if ([KonWake]::Poll() -gt 0) { Emit @{ type = 'wake' } } } catch {} }
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
