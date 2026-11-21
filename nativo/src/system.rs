//! Sistema: CPU, memória, disco C: e os processos mais pesados (lidos a cada 2 s numa thread).

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct Proc { pub name: String, pub mem_mb: u64, pub cpu: f32 }

#[derive(Clone, Debug, Default)]
pub struct SysInfo {
    pub cpu: f32, pub cores: usize,
    pub mem_used: u64, pub mem_total: u64,
    pub disk_free: u64, pub disk_total: u64,
    pub top: Vec<Proc>,
    pub uptime_s: u64,
    pub alert: Option<String>,
}

pub fn start(active: Arc<std::sync::atomic::AtomicBool>, alerts: Arc<std::sync::atomic::AtomicBool>, repaint: impl Fn() + Send + 'static) -> Arc<Mutex<SysInfo>> {
    let st = Arc::new(Mutex::new(SysInfo::default()));
    let s2 = st.clone();
    std::thread::spawn(move || {
        use sysinfo::{Disks, ProcessesToUpdate, System};
        let mut sys = System::new();
        let mut disks = Disks::new_with_refreshed_list();
        let mut high_since: Option<std::time::Instant> = None;
        let mut last_alert: Option<std::time::Instant> = None;
        loop {
            // aviso de PC sobrecarregado (igual ao Kon atual): CPU ≥ 90% ou memória ≥ 93% por 30 s, no máx. 1 a cada 10 min.
            // Só CPU e memória: é leve. Os processos só são lidos na hora de avisar.
            if alerts.load(std::sync::atomic::Ordering::Relaxed) && !active.load(std::sync::atomic::Ordering::Relaxed) {
                sys.refresh_cpu_usage(); sys.refresh_memory();
                let cpu = sys.global_cpu_usage();
                let mem = if sys.total_memory() > 0 { 100.0 * sys.used_memory() as f32 / sys.total_memory() as f32 } else { 0.0 };
                if cpu >= 90.0 || mem >= 93.0 {
                    let t0 = *high_since.get_or_insert_with(std::time::Instant::now);
                    if t0.elapsed().as_secs() >= 30 && last_alert.map_or(true, |l| l.elapsed().as_secs() >= 600) {
                        last_alert = Some(std::time::Instant::now());
                        sys.refresh_processes(ProcessesToUpdate::All, true);
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        sys.refresh_processes(ProcessesToUpdate::All, true);
                        let cores = sys.cpus().len().max(1) as f32;
                        let mut ps: Vec<_> = sys.processes().values().map(|p| (p.name().to_string_lossy().trim_end_matches(".exe").to_string(), p.cpu_usage() / cores, p.memory())).collect();
                        let text = if cpu >= 90.0 {
                            ps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                            format!("PC sobrecarregado: CPU em {:.0}%{}", cpu, ps.first().map(|p| format!(" · {} usa {:.0}%", p.0, p.1)).unwrap_or_default())
                        } else {
                            ps.sort_by(|a, b| b.2.cmp(&a.2));
                            format!("Memória quase cheia ({:.0}%){}", mem, ps.first().map(|p| format!(" · {} usa {}", p.0, gb(p.2))).unwrap_or_default())
                        };
                        s2.lock().unwrap().alert = Some(text);
                        repaint();
                    }
                } else { high_since = None; }
            }
            // só mede quando a página Sistema está aberta (medir custa CPU)
            if active.load(std::sync::atomic::Ordering::Relaxed) {
                sys.refresh_cpu_usage();
                sys.refresh_memory();
                sys.refresh_processes(ProcessesToUpdate::All, true);
                disks.refresh(true);
                let mut agg: std::collections::HashMap<String, Proc> = Default::default();
                for p in sys.processes().values() {
                    let name = p.name().to_string_lossy().trim_end_matches(".exe").to_string();
                    let e = agg.entry(name.clone()).or_insert(Proc { name, ..Default::default() });
                    e.mem_mb += p.memory() / 1_048_576;
                    e.cpu += p.cpu_usage();
                }
                let cores = sys.cpus().len().max(1);
                let mut top: Vec<Proc> = agg.into_values().map(|mut p| { p.cpu /= cores as f32; p }).collect();
                // empate em 0% de CPU (1ª leitura): os que mais usam memória
                top.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal).then(b.mem_mb.cmp(&a.mem_mb)));
                top.truncate(4);
                let c = disks.iter().find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with("C:")).or(disks.iter().next());
                let mut g = s2.lock().unwrap();
                let alert = g.alert.take();
                *g = SysInfo {
                    cpu: sys.global_cpu_usage(), cores,
                    mem_used: sys.used_memory(), mem_total: sys.total_memory(),
                    disk_free: c.map_or(0, |d| d.available_space()), disk_total: c.map_or(0, |d| d.total_space()),
                    top, uptime_s: System::uptime(), alert,
                };
                drop(g);
                repaint();
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });
    st
}

fn gb(b: u64) -> String { format!("{:.1} GB", b as f64 / 1_073_741_824.0).replace('.', ",") }
